"""Reconcile complete-session renderer events and summarize completion windows."""

import argparse
import csv
from datetime import datetime, timedelta
import gzip
import hashlib
import io
import json
from pathlib import Path
import re


EVENT = re.compile(r"^(\S+).*\brender_timing\s+request_id=(\d+)\s+queue_us=(\d+)\s+call_export_us=(\d+)\s+ok=(true|false)\s*$")
COUNTER = re.compile(r"^RUBICON_DIAGNOSTICS_DROPPED lines=(\d+)\s*$")


def load(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def save(path, value):
    path.write_text(json.dumps(value, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def distribution(values):
    if not values:
        return None
    values = sorted(values)
    def quantile(p):
        position = (len(values) - 1) * p
        lower = int(position)
        upper = min(lower + 1, len(values) - 1)
        return values[lower] + (values[upper] - values[lower]) * (position - lower)
    return {"count": len(values), "mean": sum(values) / len(values), "min": values[0],
            "p50": quantile(.5), "p95": quantile(.95), "p99": quantile(.99), "max": values[-1]}


def analyze(directory, replay=False):
    run, client = load(directory / "run.json"), load(directory / "summary.json")
    compact = directory / "timings.csv.gz"
    raw = directory / "raw/server.stderr.log"
    events, counters, errors = [], [], []
    if replay:
        previous = load(directory / "diagnostics-summary.json")
        counters = previous["final_dropped_counters"]
        raw_hash = previous["artifacts"]["server_stderr_sha256"]
        for row in csv.DictReader(io.StringIO(gzip.decompress(compact.read_bytes()).decode("utf-8"))):
            events.append({"request_id": row["request_id"], "utc": row["utc"],
                           "queue_us": int(row["queue_us"]), "call_export_us": int(row["call_export_us"]),
                           "ok": row["ok"] == "true"})
    else:
        for line in raw.read_text(encoding="utf-8", errors="strict").splitlines():
            match = EVENT.match(line)
            if match:
                stamp, request_id, queued, exported, ok = match.groups()
                events.append({"request_id": request_id, "utc": stamp, "queue_us": int(queued),
                               "call_export_us": int(exported), "ok": ok == "true"})
            elif "render_timing" in line:
                errors.append("Malformed timing event")
            counter = COUNTER.match(line)
            if counter:
                counters.append(int(counter.group(1)))
        output = io.StringIO(newline="")
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow(["request_id", "utc", "queue_us", "call_export_us", "ok"])
        writer.writerows([e["request_id"], e["utc"], e["queue_us"], e["call_export_us"], str(e["ok"]).lower()] for e in events)
        compact.write_bytes(gzip.compress(output.getvalue().encode("utf-8"), mtime=0))
        raw_hash = digest(raw)
    if run["shutdown"]["forced"] or run["shutdown"]["server_exit_code"] != 0:
        errors.append("Abnormal server shutdown")
    if run["shutdown"]["console_members"] != run["shutdown"]["console_members_at_signal"]:
        errors.append("Private console membership changed")
    if not client["valid"]:
        errors.append("Invalid client measurement")
    instrumented = run["variant"] == "on"
    phases = [client["warmup"]["outcomes"], client["outcomes"]]
    if instrumented and any(any(outcome not in {"correct", "http_503"} and count for outcome, count in phase.items()) for phase in phases):
        errors.append("Unexpected outcomes prevent exact processed-event reconciliation")
    expected = run["control_completed"] + sum(phase.get("correct", 0) for phase in phases)
    if instrumented:
        if counters != [0]:
            errors.append("Missing, duplicated, or nonzero final drop counter")
        if len(events) != expected:
            errors.append(f"Processed-event coverage mismatch: expected {expected}, found {len(events)}")
        if len({e["request_id"] for e in events}) != len(events):
            errors.append("Duplicate request identifiers in timing records")
        if not all(e["ok"] for e in events):
            errors.append("Export failures occurred")
    elif events or counters:
        errors.append("Feature-disabled binary emitted diagnostic records")
    start = datetime.fromisoformat(client["phase_start_utc"])
    end = start + timedelta(seconds=run["measurement_seconds"])
    window = [e for e in events if start <= datetime.fromisoformat(e["utc"]) < end]
    def intervals(rows):
        return {"queue_ms": distribution([e["queue_us"] / 1000 for e in rows]),
                "call_export_ms": distribution([e["call_export_us"] / 1000 for e in rows])}
    result = {"id": run["id"], "variant": run["variant"], "rate": run["rate"],
              "repetition": run["repetition"], "valid": not errors, "errors": errors,
              "scope": "Export-completion window; no per-request join to client latency",
              "expected_session_processed_events": expected, "recorded_session_events": len(events),
              "final_dropped_counters": counters, "window_events": len(window),
              "window": intervals(window), "all_session": intervals(events),
              "artifacts": {"server_stderr_sha256": raw_hash, "timings_sha256": digest(compact)}}
    save(directory / "diagnostics-summary.json", result)
    print(f"DIAGNOSTIC_RESULT {run['id']}: events={len(events)}/{expected if instrumented else 0} drops={counters} valid={not errors}")
    if errors:
        raise RuntimeError("; ".join(errors))
    return result


def aggregate(root):
    records = []
    for directory in sorted(root.iterdir()):
        if not directory.is_dir():
            continue
        client, internal = load(directory / "summary.json"), load(directory / "diagnostics-summary.json")
        records.append({"id": client["id"], "variant": client["variant"], "rate": client["offered_rps"],
                        "repetition": client["repetition"], "client": client, "internal": internal})
    pairs = []
    for rate, repetition in sorted({(r["rate"], r["repetition"]) for r in records}):
        group = {r["variant"]: r for r in records if r["rate"] == rate and r["repetition"] == repetition}
        off, on = group["off"]["client"], group["on"]["client"]
        pairs.append({"rate": rate, "repetition": repetition,
                      "correct_rps_delta_on_minus_off": on["correct_rps_in_window"] - off["correct_rps_in_window"],
                      "p95_ms_delta_on_minus_off": on["latency_correct_ms"]["p95"] - off["latency_correct_ms"]["p95"],
                      "server_cpu_percentage_points_delta": on["server"]["allocated_cpu_mean_percent"] - off["server"]["allocated_cpu_mean_percent"]})
    save(root / "diagnostics-results.json", {"runs": records, "pairs": pairs,
         "interpretation": "Descriptive paired differences from two repetitions; not a precise instrumentation-cost estimate"})
    with (root / "diagnostics-results.csv").open("w", newline="", encoding="utf-8") as output:
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow(["id", "rate", "variant", "repetition", "correct_rps", "client_p95_ms", "http_503", "queue_p50_ms", "queue_p95_ms", "export_p50_ms", "export_p95_ms", "events", "drops", "valid"])
        for record in records:
            client, internal = record["client"], record["internal"]
            queued, exported = internal["window"]["queue_ms"], internal["window"]["call_export_ms"]
            writer.writerow([record["id"], record["rate"], record["variant"], record["repetition"], client["correct_rps_in_window"], client["latency_correct_ms"]["p95"], client["outcomes"].get("http_503", 0), queued["p50"] if queued else "", queued["p95"] if queued else "", exported["p50"] if exported else "", exported["p95"] if exported else "", internal["recorded_session_events"], internal["final_dropped_counters"], internal["valid"]])
    print(f"DIAGNOSTIC_AGGREGATE {len(records)} runs")


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
        analyze(args.directory, replay=args.mode == "--replay")
