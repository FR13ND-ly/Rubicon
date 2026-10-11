"""Adversarial checks using a real stage smoke; preserve mutated inputs and summaries."""

import argparse
import gzip
import importlib.util
import io
import csv
from datetime import timedelta
from pathlib import Path
import shutil


ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("stage_analysis", Path(__file__).with_name("analyze-stages.py"))
stage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stage)


def copy_inputs(source, target):
    (target / "raw").mkdir(parents=True)
    for name in ("run.json", "summary.json"):
        shutil.copyfile(source / name, target / name)
    shutil.copyfile(source / "raw/server.stderr.log", target / "raw/server.stderr.log")


def handled_error_checks():
    results = []
    for name, sequence in (("prepare-failure", stage.FAILURES[0]), ("execute-failure", stage.FAILURES[1]),
                           ("conversion-failure", stage.FAILURES[2]), ("success", stage.SUCCESS)):
        lines = [f'2026-10-11T09:00:00.00{number}000Z INFO render{{request_id=1}}: runtime_stage_timing stage="{label}" elapsed_us=10 ok={str(ok).lower()}'
                 for number, (label, ok) in enumerate(sequence, 1)]
        lines.append(f'2026-10-11T09:00:00.010000Z INFO render_timing request_id=1 queue_us=1 call_export_us=100 ok={str(name == "success").lower()}')
        lines.append('RUBICON_DIAGNOSTICS_DROPPED lines=0')
        records, parse_errors = stage.parsed_records("\n".join(lines))
        joined, errors = stage.join_records(records)
        if parse_errors or errors or len(joined) != 1:
            raise AssertionError((name, parse_errors, errors))
        results.append({"case": name, "valid_stage_sequence": True,
                        "stages": len(sequence), "coarse_ok": name == "success"})
    return results


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("smoke_run", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--off-run", type=Path, required=True)
    args = parser.parse_args()
    source, output, off = args.smoke_run.resolve(strict=True), args.output.resolve(), args.off_run.resolve(strict=True)
    if any(not path.is_relative_to(ROOT) for path in (source, output, off)) or output.exists():
        raise ValueError("Use repository smoke inputs and a fresh repository output directory")
    output.mkdir(parents=True)
    original = (source / "raw/server.stderr.log").read_text(encoding="utf-8")
    lines = original.splitlines()
    first = [line for line in lines if (match := stage.STAGE.match(line)) and match.group(2) == "1"]
    if len(first) != 3:
        raise AssertionError("Expected all three stages of the first request")
    exported = next(line for line in lines if (match := stage.coarse.EVENT.match(line)) and match.group(2) == "1")
    export_match = stage.coarse.EVENT.match(exported)
    execute_match = stage.STAGE.match(first[1])
    after = (stage.utc(export_match.group(1)) + timedelta(seconds=1)).isoformat()
    cases = {f"missing-{label}": original.replace(first[index] + "\n", "", 1) for index, label in enumerate(stage.STAGES)}
    cases.update({
        "missing-all-stages": "\n".join(line for line in lines if "runtime_stage_timing" not in line) + "\n",
        "duplicate-stage": original.replace(first[0], first[0] + "\n" + first[0], 1),
        "orphan-stage": original.replace(first[0], first[0] + "\n" + first[0].replace("request_id=1}", "request_id=999999}"), 1),
        "reordered-stages": original.replace(first[0], first[1], 1).replace(first[1] + "\n" + first[1], first[1] + "\n" + first[0], 1),
        "unknown-stage": original.replace(first[0], first[0].replace('stage="prepare"', 'stage="unknown"'), 1),
        "stage-failure-with-successful-export": original.replace(first[0], first[0].replace("ok=true", "ok=false"), 1),
        "stage-sum-exceeds-export": original.replace(first[1], first[1].replace("elapsed_us=" + execute_match.group(4), "elapsed_us=" + str(int(export_match.group(4)) + 1000)), 1),
        "stage-timestamp-after-export": original.replace(first[2], first[2].replace(stage.STAGE.match(first[2]).group(1), after, 1), 1),
        "stage-after-drop-counter": original + first[0] + "\n",
        "malformed-stage": original.replace(first[0], first[0].replace("elapsed_us=", "elapsed_invalid="), 1),
    })
    # Exchange complete groups between two IDs. The global number of stages
    # remains correct; each group's time and duration must match its own export.
    exchanged = original.replace("request_id=1}:", "request_id=TEMP}:").replace("request_id=2}:", "request_id=1}:").replace("request_id=TEMP}:", "request_id=2}:")
    cases["stages-associated-with-wrong-request"] = exchanged
    results = []
    for name, log in cases.items():
        if log == original:
            raise AssertionError(f"Mutation did not change input: {name}")
        directory = output / name
        copy_inputs(source, directory)
        (directory / "raw/server.stderr.log").write_text(log, encoding="utf-8")
        stage.coarse.analyze(directory)
        try:
            stage.analyze(directory)
        except RuntimeError as error:
            summary = stage.coarse.load(directory / "stage-summary.json")
            if summary["valid"]:
                raise AssertionError("Rejected fixture must retain an invalid summary")
            before = (directory / "stage-summary.json").read_bytes()
            try:
                stage.analyze(directory, replay=True)
            except RuntimeError:
                pass
            else:
                raise AssertionError("Replay accepted an invalid stage fixture")
            if before != (directory / "stage-summary.json").read_bytes():
                raise AssertionError(f"Invalid fixture replay changed its summary: {name}")
            results.append({"case": name, "rejected": True, "replay_byte_identical": True, "reason": str(error)})
        else:
            raise AssertionError(f"Invalid stage fixture was accepted: {name}")
    directory = output / "feature-disabled-with-stage"
    copy_inputs(off, directory)
    with (directory / "raw/server.stderr.log").open("a", encoding="utf-8") as handle:
        handle.write(first[0] + "\n")
    stage.coarse.analyze(directory)
    try:
        stage.analyze(directory)
    except RuntimeError as error:
        results.append({"case": "feature-disabled-with-stage", "rejected": True, "reason": str(error)})
    else:
        raise AssertionError("Disabled variant accepted a runtime stage")
    directory = output / "legacy-schema-refused"
    copy_inputs(source, directory)
    run = stage.coarse.load(directory / "run.json")
    del run["runtime_stage_schema"]
    stage.coarse.save(directory / "run.json", run)
    try:
        stage.analyze(directory)
    except ValueError as error:
        if (directory / "runtime-events.csv.gz").exists() or (directory / "stage-summary.json").exists():
            raise AssertionError("Legacy-schema refusal wrote stage artifacts")
        results.append({"case": "legacy-schema-refused", "rejected_before_writing": True, "reason": str(error)})
    else:
        raise AssertionError("Undeclared stage schema was accepted")
    directory = output / "compact-corruption"
    copy_inputs(source, directory)
    stage.coarse.analyze(directory)
    stage.analyze(directory)
    compact = directory / "runtime-events.csv.gz"
    payload = gzip.decompress(compact.read_bytes()).decode("utf-8")
    reader = csv.DictReader(io.StringIO(payload))
    first_row = next(reader)
    first_row["elapsed_us"] = str(int(first_row["elapsed_us"]) + 1)
    stream = io.StringIO(newline="")
    writer = csv.DictWriter(stream, stage.FIELDS, lineterminator="\n")
    writer.writeheader(); writer.writerow(first_row); writer.writerows(reader)
    compact.write_bytes(gzip.compress(stream.getvalue().encode("utf-8"), mtime=0))
    preserved_summary = (directory / "stage-summary.json").read_bytes()
    try:
        stage.analyze(directory, replay=True)
    except ValueError as error:
        if "Replay runtime compact hash changed" not in str(error):
            raise AssertionError("Changed compact must be detected by its hash")
        if preserved_summary != (directory / "stage-summary.json").read_bytes():
            raise AssertionError("Hash refusal overwrote the preserved summary")
        results.append({"case": "compact-corruption", "rejected_before_writing": True, "reason": str(error)})
    else:
        raise AssertionError("Changed compact input was accepted")
    directory = output / "replay-metadata-corruption"
    copy_inputs(source, directory)
    stage.coarse.analyze(directory)
    stage.analyze(directory)
    preserved_summary = (directory / "stage-summary.json").read_bytes()
    run = stage.coarse.load(directory / "run.json")
    run["first_request"]["client_elapsed_ms"] += 1
    stage.coarse.save(directory / "run.json", run)
    try:
        stage.analyze(directory, replay=True)
    except ValueError as error:
        if "Replay input hash changed: run_sha256" not in str(error):
            raise AssertionError("Changed run metadata must be detected by its hash")
        if preserved_summary != (directory / "stage-summary.json").read_bytes():
            raise AssertionError("Metadata refusal overwrote the preserved summary")
        results.append({"case": "replay-metadata-corruption", "rejected_before_writing": True, "reason": str(error)})
    else:
        raise AssertionError("Changed run metadata was accepted")
    results.extend(handled_error_checks())
    stage.coarse.save(output / "checks.json", results)
    print(f"STAGE_CHECKS_PASS cases={len(results)}")


if __name__ == "__main__":
    main()
