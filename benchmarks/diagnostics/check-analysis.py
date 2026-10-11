"""Negative checks against a real smoke run; preserve each altered fixture."""

import argparse
import importlib.util
import json
from pathlib import Path
import shutil


ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("diagnostics_analysis", Path(__file__).with_name("analyze.py"))
analysis = importlib.util.module_from_spec(spec)
spec.loader.exec_module(analysis)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("smoke_run", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    source, output = args.smoke_run.resolve(strict=True), args.output.resolve()
    if not source.is_relative_to(ROOT) or not output.is_relative_to(ROOT) or output.exists():
        raise ValueError("Use an existing repository smoke run and a fresh repository output path")
    output.mkdir()
    original = (source / "raw/server.stderr.log").read_text(encoding="utf-8")
    first = next(line for line in original.splitlines() if "render_timing" in line)
    cases = {
        "missing-event": original.replace(first + "\n", "", 1),
        "duplicate-event": original.replace(first, first + "\n" + first, 1),
        "nonzero-drop": original.replace("RUBICON_DIAGNOSTICS_DROPPED lines=0", "RUBICON_DIAGNOSTICS_DROPPED lines=1"),
        "missing-counter": original.replace("RUBICON_DIAGNOSTICS_DROPPED lines=0\n", ""),
    }
    results = []
    for name, log in cases.items():
        if log == original:
            raise AssertionError("Negative fixture mutation did not change input")
        directory = output / name
        (directory / "raw").mkdir(parents=True)
        for file in ("run.json", "summary.json"):
            shutil.copyfile(source / file, directory / file)
        (directory / "raw/server.stderr.log").write_text(log, encoding="utf-8")
        try:
            analysis.analyze(directory)
        except RuntimeError as error:
            results.append({"case": name, "rejected": True, "reason": str(error)})
        else:
            raise AssertionError(f"Incorrect diagnostic fixture was accepted: {name}")
    # A client timeout remains a measured external failure. It blocks exact
    # event reconciliation only when the variant actually emits events.
    for variant in ("off", "on"):
        directory = output / f"transport-{variant}"
        (directory / "raw").mkdir(parents=True)
        run = analysis.load(source / "run.json")
        client = analysis.load(source / "summary.json")
        run["variant"] = variant
        client["outcomes"]["correct"] -= 1
        client["outcomes"]["transport"] = 1
        analysis.save(directory / "run.json", run)
        analysis.save(directory / "summary.json", client)
        (directory / "raw/server.stderr.log").write_text(original if variant == "on" else "", encoding="utf-8")
        try:
            analysis.analyze(directory)
        except RuntimeError as error:
            if variant != "on":
                raise AssertionError("Feature-disabled client failures must remain measurable") from error
            results.append({"case": f"transport-{variant}", "rejected": True, "reason": str(error)})
        else:
            if variant != "off":
                raise AssertionError("Instrumented timeout prevents exact event reconciliation")
            results.append({"case": f"transport-{variant}", "accepted_external_observation": True})
    analysis.save(output / "checks.json", results)
    print(f"DIAGNOSTIC_NEGATIVE_CHECKS_PASS cases={len(results)}")


if __name__ == "__main__":
    main()
