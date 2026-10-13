import json
import math
from pathlib import Path


OUTCOMES = {
    "correct": "correct_responses",
    "unavailable": "unavailable_responses",
    "http_error": "other_http_errors",
    "transport_error": "transport_errors",
    "invalid_html": "invalid_html_responses",
}


def number(value):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError("Expected a numeric metric")
    if not math.isfinite(value) or value < 0:
        raise ValueError("Expected a finite, nonnegative metric")
    return value


def measurement(directory, backend, vus, expected_requests, exit_code, error=None):
    result = {
        "directory": str(directory), "backend": backend, "vus": vus,
        "exit_code": exit_code, "valid": False, "problems": [],
    }
    if exit_code != 0:
        result["problems"].append(f"Runner exited with code {exit_code}")
    if error:
        result["problems"].append(error)
    try:
        metrics = json.loads((directory / "measurement.k6.json").read_text())["metrics"]
        resources = json.loads((directory / "measurement.resources.json").read_text())
        count = number(metrics["http_reqs"]["count"])
        rate = number(metrics["http_reqs"]["rate"])
        if count != int(count) or count == 0 or rate == 0:
            raise ValueError("Missing completed HTTP requests or measurement duration")
        counts = {name: number(metrics[key]["count"]) for name, key in OUTCOMES.items()}
        if any(value != int(value) for value in counts.values()):
            raise ValueError("Response counts must be integers")
        result.update(requests=int(count), counts=counts, k6_window_s=count / rate)
        result["resources"] = {}
        for name in ("server", "cli", "k6", "coordinator"):
            raw = resources["summary"][name]
            if not isinstance(raw["cpu_time_complete"], bool):
                raise ValueError("CPU completeness must be explicit")
            result["resources"][name] = {
                "cpu_seconds": number(raw["cpu_seconds"]),
                "cpu_mean_percent": number(raw["cpu_mean_percent"]),
                "rss_peak_mib": number(raw["rss_peak_mib"]),
                "private_peak_mib": None if raw["private_peak_mib"] is None else number(raw["private_peak_mib"]),
                "cpu_time_complete": raw["cpu_time_complete"],
            }
        if resources["phase"] != "Measurement" or resources["backend"] != backend:
            result["problems"].append("Resource data belongs to another phase or backend")
        if resources["vus"] != vus or resources["requested_requests"] != expected_requests:
            result["problems"].append("Resource data does not match the planned load")
        if resources["k6_exit_code"] != 0 or resources.get("error"):
            result["problems"].append("Measurement did not finish successfully")
        if count != expected_requests or sum(counts.values()) != count:
            result["problems"].append("Request accounting is incomplete or inconsistent")
        if counts["correct"] != count:
            result["problems"].append("Some responses failed correctness checks")
        result["latency"] = {}
        if counts["correct"]:
            duration = metrics["correct_response_duration"]
            result["latency"] = {
                "mean_ms": number(duration["avg"]),
                "p95_ms": number(duration["p(95)"]),
                "p99_ms": number(duration["p(99)"]),
                "ttfb_mean_ms": number(metrics["correct_response_ttfb"]["avg"]),
            }
    except (OSError, ValueError, KeyError, TypeError) as exception:
        result["problems"].append(f"Incomplete measurement: {exception}")
    result["valid"] = not result["problems"]
    return result


def group_results(runs):
    groups = []
    keys = sorted({(run["vus"], run["backend"]) for run in runs})
    for vus, backend in keys:
        members = [run for run in runs if run["vus"] == vus and run["backend"] == backend]
        observed = [run for run in members if "k6_window_s" in run]
        counts = {name: sum(run["counts"][name] for run in observed) for name in OUTCOMES}
        elapsed = sum(run["k6_window_s"] for run in observed)
        correct = counts["correct"]
        group = {
            "vus": vus, "backend": backend, "runs": len(members),
            "all_valid": all(run["valid"] for run in members),
            "requests": sum(run["requests"] for run in observed), "counts": counts,
            "correct_rps": correct / elapsed if elapsed else None,
            "mean_ms": None, "ttfb_mean_ms": None,
            "p95_ms_per_run": [], "p99_ms_per_run": [], "resources": {},
        }
        timed = [run for run in observed if run.get("latency")]
        if correct and sum(run["counts"]["correct"] for run in timed) == correct:
            for key in ("mean_ms", "ttfb_mean_ms"):
                group[key] = sum(run["latency"][key] * run["counts"]["correct"] for run in timed) / correct
        for key in ("p95_ms", "p99_ms"):
            group[f"{key}_per_run"] = [run["latency"][key] for run in timed]
        for name in ("server", "cli", "k6", "coordinator"):
            values = [run["resources"][name] for run in observed if name in run.get("resources", {})]
            if not values:
                continue
            complete = len(values) == len(members) and all(value["cpu_time_complete"] for value in values)
            cpu = sum(value["cpu_seconds"] for value in values)
            group["resources"][name] = {
                "cpu_seconds": cpu, "cpu_time_complete": complete,
                "cpu_ms_per_correct_request": cpu * 1000 / correct if complete and correct else None,
                "cpu_mean_percent_per_run": [value["cpu_mean_percent"] for value in values],
                "rss_peak_mib_per_run": [value["rss_peak_mib"] for value in values],
                "private_peak_mib_per_run": [value["private_peak_mib"] for value in values],
            }
        groups.append(group)
    return groups


def display(value):
    return "n/a" if value is None else f"{value:.2f}"


def interval(values):
    return "n/a" if not values else f"{min(values):.2f}-{max(values):.2f}"


def write_report(directory, runs, manifest):
    groups = group_results(runs)
    valid = (
        manifest["completed"] and manifest["artifacts_unchanged"]
        and len(runs) == len(manifest["run_order"]) and all(run["valid"] for run in runs)
    )
    report = {
        "schema_version": 1, "completed": manifest["completed"], "all_valid": valid,
        "artifacts_unchanged": manifest["artifacts_unchanged"], "runs": runs, "groups": groups,
        "definitions": {
            "throughput": "Correct response count divided by the sum of k6 execution windows; not the mean of run rates.",
            "latency": "Mean weighted by correct responses. p95 and p99 are retained per run; they are not pooled or averaged.",
            "ttfb": "k6 HTTP waiting time for correct responses, excluding connection establishment.",
            "cpu": "User + system time over each full measurement phase, divided by correct responses. Incomplete CPU totals produce n/a.",
            "ram": "Maximum sampled server-tree RSS per run, in MiB. Shared pages may be counted more than once; brief peaks may be missed.",
            "validity": "Errors and incomplete runs are retained. An invalid or unfinished suite does not establish a performance advantage.",
        },
    }
    directory.joinpath("report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    lines = [
        "# Products comparison", "", f"Completed: {manifest['completed']}. All runs valid: {valid}.", "",
        "Configured server artifacts (see manifest); sequential servers, alternating order, fresh server per run.",
        "Fixed VUs with finite iterations. CPU and RAM are observed; no resource caps are applied.", "",
        "| Backend | VUs | Runs | Valid | Correct/s | Mean ms | TTFB ms | p95 range ms | p99 range ms | CPU ms/correct | RSS peak MiB | Incorrect |",
        "|---|---:|---:|---|---:|---:|---:|---|---|---:|---:|---:|",
    ]
    for group in groups:
        server = group["resources"].get("server", {})
        peaks = server.get("rss_peak_mib_per_run", [])
        errors = sum(value for name, value in group["counts"].items() if name != "correct")
        lines.append(
            f"| {group['backend']} | {group['vus']} | {group['runs']} | {group['all_valid']} | "
            f"{display(group['correct_rps'])} | {display(group['mean_ms'])} | {display(group['ttfb_mean_ms'])} | "
            f"{interval(group['p95_ms_per_run'])} | {interval(group['p99_ms_per_run'])} | "
            f"{display(server.get('cpu_ms_per_correct_request'))} | {display(max(peaks) if peaks else None)} | {errors} |"
        )
    lines.extend(["", "Percentile ranges show the separate runs. CPU 100% means one logical CPU; it may exceed 100%.", ""])
    for run in runs:
        if run["problems"]:
            lines.append(f"- {Path(run['directory']).name}: {'; '.join(run['problems'])}")
    directory.joinpath("report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return report
