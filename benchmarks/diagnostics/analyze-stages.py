"""Join runtime stages to their enclosing exports without changing legacy summaries."""

import argparse
import csv
from datetime import datetime, timedelta
import gzip
import importlib.util
import io
from pathlib import Path
import re


spec = importlib.util.spec_from_file_location("coarse_analysis", Path(__file__).with_name("analyze.py"))
coarse = importlib.util.module_from_spec(spec)
spec.loader.exec_module(coarse)

STAGE = re.compile(r'^(\S+).*\brender\{request_id=(\d+)\}:\s+runtime_stage_timing\s+stage="([^\"]+)"\s+elapsed_us=(\d+)\s+ok=(true|false)\s*$')
FIELDS = ["order", "kind", "utc", "request_id", "stage", "elapsed_us", "queue_us", "call_export_us", "ok"]
STAGES = ("prepare", "execute", "convert")
SUCCESS = [(name, True) for name in STAGES]
FAILURES = [[("prepare", False)], [("prepare", True), ("execute", False)],
            [("prepare", True), ("execute", True), ("convert", False)]]


def utc(value):
    result = datetime.fromisoformat(value)
    if result.tzinfo is None or result.utcoffset() != timedelta(0):
        raise ValueError("Timing timestamps must identify UTC")
    return result


def parsed_records(source):
    records, errors = [], []
    counter_lines = []
    for number, line in enumerate(source.splitlines(), 1):
        if coarse.COUNTER.match(line):
            counter_lines.append(number)
        match = STAGE.match(line)
        if match:
            stamp, identifier, stage, elapsed, ok = match.groups()
            records.append({"order": number, "kind": "stage", "utc": stamp, "request_id": identifier,
                            "stage": stage, "elapsed_us": int(elapsed), "queue_us": None,
                            "call_export_us": None, "ok": ok == "true"})
        else:
            match = coarse.EVENT.match(line)
            if match:
                stamp, identifier, queued, exported, ok = match.groups()
                records.append({"order": number, "kind": "export", "utc": stamp, "request_id": identifier,
                                "stage": "", "elapsed_us": None, "queue_us": int(queued),
                                "call_export_us": int(exported), "ok": ok == "true"})
            elif "runtime_stage_timing" in line or "render_timing" in line:
                errors.append(f"Malformed timing event at line {number}: {line}")
    if len(counter_lines) == 1 and any(record["order"] > counter_lines[0] for record in records):
        errors.append("Timing event occurs after the final writer-drop counter")
    return records, errors


def save_records(path, records):
    output = io.StringIO(newline="")
    writer = csv.DictWriter(output, FIELDS, lineterminator="\n")
    writer.writeheader()
    for record in records:
        writer.writerow(dict(record, ok=str(record["ok"]).lower()))
    path.write_bytes(gzip.compress(output.getvalue().encode("utf-8"), mtime=0))


def load_records(path):
    reader = csv.DictReader(io.StringIO(gzip.decompress(path.read_bytes()).decode("utf-8")))
    if reader.fieldnames != FIELDS:
        raise ValueError("Unsupported runtime-event compact schema")
    records = []
    for row in reader:
        if row["kind"] not in {"stage", "export"} or row["ok"] not in {"true", "false"}:
            raise ValueError("Invalid compact runtime event")
        for name in ("order", "elapsed_us", "queue_us", "call_export_us"):
            row[name] = int(row[name]) if row[name] else None
        row["ok"] = row["ok"] == "true"
        records.append(row)
    return records


def join_records(records):
    """Validate both successful and handled-error sequences, retaining every export."""
    errors, exports, by_id = [], [], {}
    previous_order = 0
    for record in records:
        if record["order"] <= previous_order:
            errors.append("Runtime records are not in their original log order")
        previous_order = record["order"]
        try:
            utc(record["utc"])
        except ValueError:
            errors.append(f"Invalid UTC timestamp for request {record['request_id']}")
        if record["kind"] == "stage":
            if record["stage"] not in STAGES:
                errors.append(f"Unknown runtime stage: {record['stage']}")
            if record["elapsed_us"] is None or record["elapsed_us"] < 0:
                errors.append("Missing or negative stage duration")
            by_id.setdefault(record["request_id"], []).append(record)
        elif record["kind"] == "export":
            exports.append(record)
            if any(record[name] is None or record[name] < 0 for name in ("queue_us", "call_export_us")):
                errors.append("Missing or negative export duration")
        else:
            errors.append("Unknown runtime event kind")
    export_ids = [record["request_id"] for record in exports]
    if len(set(export_ids)) != len(export_ids):
        errors.append("Duplicate enclosing export identifiers")
    for identifier in sorted(set(by_id) - set(export_ids)):
        errors.append(f"Orphan runtime stages for request {identifier}")
    joined = []
    for exported in exports:
        identifier = exported["request_id"]
        stages = by_id.get(identifier, [])
        sequence = [(record["stage"], record["ok"]) for record in stages]
        accepted = sequence == SUCCESS if exported["ok"] else sequence in FAILURES
        if not accepted:
            errors.append(f"Invalid runtime stage sequence for request {identifier}: {sequence}")
        if any(record["order"] >= exported["order"] for record in stages):
            errors.append(f"Stage emitted after enclosing export for request {identifier}")
        try:
            stamps = [utc(record["utc"]) for record in stages] + [utc(exported["utc"])]
            if stamps != sorted(stamps):
                errors.append(f"Stage timestamps are out of order for request {identifier}")
        except ValueError:
            pass  # The invalid timestamp was already retained above.
        durations = {record["stage"]: record["elapsed_us"] for record in stages}
        numeric = all(record["elapsed_us"] is not None and record["elapsed_us"] >= 0 for record in stages)
        stage_sum = sum(record["elapsed_us"] for record in stages) if numeric else None
        if stage_sum is not None and exported["call_export_us"] is not None and stage_sum > exported["call_export_us"]:
            errors.append(f"Stage sum exceeds enclosing call for request {identifier}")
        joined.append({"export": exported, "stages": stages, "durations": durations,
                       "stage_sum_us": stage_sum,
                       "successful_sequence": sequence == SUCCESS and exported["ok"] and numeric
                       and exported["call_export_us"] is not None})
    return joined, errors


def coarse_records(path):
    reader = csv.DictReader(io.StringIO(gzip.decompress(path.read_bytes()).decode("utf-8")))
    return [{"request_id": row["request_id"], "utc": row["utc"], "queue_us": int(row["queue_us"]),
             "call_export_us": int(row["call_export_us"]), "ok": row["ok"] == "true"} for row in reader]


def intervals(rows):
    rows = [row for row in rows if row["successful_sequence"]]
    result = {f"{stage}_ms": coarse.distribution([row["durations"][stage] / 1000 for row in rows]) for stage in STAGES}
    result["stage_sum_ms"] = coarse.distribution([row["stage_sum_us"] / 1000 for row in rows])
    result["other_call_ms"] = coarse.distribution([
        (row["export"]["call_export_us"] - row["stage_sum_us"]) / 1000 for row in rows])
    return result


def analyze(directory, replay=False):
    run = coarse.load(directory / "run.json")
    if run.get("runtime_stage_schema") != 1:
        raise ValueError("Run must explicitly declare runtime_stage_schema=1; legacy results are not stage experiments")
    client = coarse.load(directory / "summary.json")
    legacy_path = directory / "diagnostics-summary.json"
    legacy = coarse.load(legacy_path)
    legacy_compact = directory / "timings.csv.gz"
    compact = directory / "runtime-events.csv.gz"
    output = directory / "stage-summary.json"
    source_hashes = {"run_sha256": coarse.digest(directory / "run.json"),
                     "client_summary_sha256": coarse.digest(directory / "summary.json"),
                     "coarse_summary_sha256": coarse.digest(legacy_path),
                     "coarse_timings_sha256": coarse.digest(legacy_compact)}
    errors = []
    if replay:
        previous = coarse.load(output)
        for name, value in source_hashes.items():
            if previous["artifacts"][name] != value:
                raise ValueError(f"Replay input hash changed: {name}; preserved summary was not overwritten")
        if previous["artifacts"]["runtime_events_sha256"] != coarse.digest(compact):
            raise ValueError("Replay runtime compact hash changed; preserved summary was not overwritten")
        records = load_records(compact)
        parser_errors = previous["parser_errors"]
        raw_hash = previous["artifacts"]["server_stderr_sha256"]
    else:
        raw = directory / "raw/server.stderr.log"
        records, parser_errors = parsed_records(raw.read_text(encoding="utf-8", errors="strict"))
        raw_hash = coarse.digest(raw)
        save_records(compact, records)
    errors.extend(parser_errors)
    if raw_hash != legacy["artifacts"]["server_stderr_sha256"]:
        errors.append("Raw log hash disagrees with coarse analysis")
    if any(legacy[name] != run[name] for name in ("id", "variant", "rate", "repetition")):
        errors.append("Coarse summary identifies a different run")
    extracted = [{name: record[name] for name in ("request_id", "utc", "queue_us", "call_export_us", "ok")}
                 for record in records if record["kind"] == "export"]
    if extracted != coarse_records(legacy_compact):
        errors.append("Runtime stream disagrees with the preserved coarse compact")
    joined, join_errors = join_records(records)
    errors.extend(join_errors)
    instrumented = run["variant"] == "on"
    if not instrumented and records:
        errors.append("Feature-disabled binary emitted runtime timing events")
    first_control = run.get("first_request")
    if not first_control or first_control.get("content_gate") != "passed":
        errors.append("Missing correctness-checked first control request")
    first = None
    if instrumented:
        expected_id = first_control.get("expected_request_id") if first_control else None
        if not joined or expected_id != "1" or joined[0]["export"]["request_id"] != expected_id:
            errors.append("First export is not the expected initial request 1")
        elif first_control:
            row = joined[0]
            try:
                start, finish = utc(first_control["started_utc"]), utc(first_control["finished_utc"])
                within = [record for record in records if record["kind"] == "export" and start <= utc(record["utc"]) <= finish]
                if len(within) != 1 or within[0]["request_id"] != expected_id:
                    errors.append("First control interval does not contain exactly its expected export")
                if any(not start <= utc(stage["utc"]) <= finish for stage in row["stages"]):
                    errors.append("First request stages lie outside the control request interval")
            except (KeyError, ValueError):
                errors.append("Invalid first-request control interval")
            first = {"request_id": expected_id, "queue_ms": row["export"]["queue_us"] / 1000,
                     "call_export_ms": row["export"]["call_export_us"] / 1000,
                     "stages_ms": {stage: value / 1000 for stage, value in row["durations"].items()},
                     "ok": row["export"]["ok"]}
    stage_join_valid = not errors
    if not legacy["valid"]:
        errors.extend(f"Coarse diagnostic: {reason}" for reason in legacy["errors"])
    start = utc(client["phase_start_utc"])
    finish = start + timedelta(seconds=run["measurement_seconds"])
    window = []
    for row in joined:
        try:
            if start <= utc(row["export"]["utc"]) < finish:
                window.append(row)
        except ValueError:
            pass
    if instrumented and len(window) != legacy["window_events"]:
        errors.append("Stage cohort disagrees with coarse completion-window count")
    result = {"schema_version": 1, "id": run["id"], "variant": run["variant"], "rate": run["rate"],
              "repetition": run["repetition"], "valid": not errors, "stage_join_valid": stage_join_valid,
              "errors": errors, "parser_errors": parser_errors,
              "scope": "Stages joined to the same server export-completion cohort; no direct client latency join",
              "recorded_stage_events": sum(record["kind"] == "stage" for record in records),
              "recorded_export_events": len(joined), "window_exports": len(window),
              "first_request_association": "Ordered first control request and initial server counter; no request-ID response header",
              "first_control": first_control, "first_render": first,
              "window": intervals(window), "all_session": intervals(joined),
              "artifacts": dict(source_hashes, server_stderr_sha256=raw_hash,
                                runtime_events_sha256=coarse.digest(compact))}
    coarse.save(output, result)
    print(f"STAGE_RESULT {run['id']}: stages={result['recorded_stage_events']} exports={len(joined)} valid={not errors}")
    if errors:
        raise RuntimeError("; ".join(errors))
    return result


def aggregate(root):
    runs = [coarse.load(directory / "stage-summary.json") for directory in sorted(root.iterdir()) if directory.is_dir()]
    coarse.save(root / "stage-results.json", {"runs": runs,
        "interpretation": "Independent short release runs; first renders are separate observations. Invalid populations remain flagged."})
    print(f"STAGE_AGGREGATE runs={len(runs)}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    modes = parser.add_mutually_exclusive_group(required=True)
    for mode in ("--run", "--replay", "--all"):
        modes.add_argument(mode, dest="mode", action="store_const", const=mode)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    if args.mode == "--all":
        aggregate(args.directory)
    else:
        try:
            analyze(args.directory, replay=args.mode == "--replay")
        except RuntimeError as error:
            print(f"STAGE_LIMITATION {error}")
            raise SystemExit(2)
        except ValueError as error:
            print(f"STAGE_INPUT_REFUSED {error}")
            raise SystemExit(1)
