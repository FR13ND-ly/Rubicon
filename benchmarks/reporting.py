import hashlib
import json
import math
from pathlib import Path
import tomllib


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


def measurement(directory, backend, vus, expected_requests, exit_code, error=None, workers=None, renderer=None, semi_space_mib=0, body_transport=None, http_threads=None):
    result = {
        "directory": str(directory), "backend": backend, "vus": vus,
        "workers": 1 if workers is None else workers,
        "semi_space_mib": semi_space_mib,
        "body_transport": body_transport or ("text" if backend == "rubicon" else "native"),
        "http_threads": http_threads, "http_threads_actual": None,
        "exit_code": exit_code, "valid": False, "problems": [],
    }
    if exit_code != 0:
        result["problems"].append(f"Runner exited with code {exit_code}")
    if error:
        result["problems"].append(error)
    if workers is not None:
        try:
            metadata = json.loads((directory / "run.json").read_text(encoding="utf-8"))
            warmup = json.loads((directory / "warmup.json").read_text(encoding="utf-8"))
            if metadata["backend"] != backend or metadata["workers"] != workers:
                result["problems"].append("Execution metadata does not match the planned backend or workers")
            if http_threads is not None:
                actual_threads = metadata.get("http_threads_actual")
                if metadata.get("http_threads") != http_threads:
                    result["problems"].append("HTTP thread profile does not match the planned variant")
                if isinstance(actual_threads, bool) or not isinstance(actual_threads, int) or actual_threads < 1:
                    result["problems"].append("Effective HTTP thread count is missing or invalid")
                else:
                    result["http_threads_actual"] = actual_threads
                    if http_threads and actual_threads != http_threads:
                        result["problems"].append("Effective HTTP thread count differs from the requested profile")
                    markers = [line for line in (directory / "server.stderr.log").read_text(encoding="utf-8").splitlines()
                               if line.startswith("RUBICON_DEMO_HTTP_THREADS")]
                    if markers != [f"RUBICON_DEMO_HTTP_THREADS={actual_threads}"]:
                        result["problems"].append("Effective HTTP thread count was not confirmed by the server")
            if metadata.get("semi_space_mib", 0) != semi_space_mib:
                result["problems"].append("V8 memory profile does not match the planned variant")
            if body_transport is not None:
                if metadata.get("body_transport") != body_transport:
                    result["problems"].append("Body transport does not match the planned variant")
                if backend == "rubicon":
                    expected_marker = f"RUBICON_DEMO_BODY_TRANSPORT={body_transport}"
                    if (directory / "server.stderr.log").read_text(encoding="utf-8").splitlines().count(expected_marker) != 1:
                        result["problems"].append("Requested body transport was not confirmed by the server")
            if semi_space_mib:
                expected_marker = f"RUBICON_DEMO_V8_SEMI_SPACE_MIB={semi_space_mib}"
                if (directory / "server.stderr.log").read_text(encoding="utf-8").splitlines().count(expected_marker) != 1:
                    result["problems"].append("Requested V8 memory profile was not confirmed by the server")
            if metadata["vus"] != vus or metadata["requests"] != expected_requests:
                result["problems"].append("Execution metadata does not match the planned load")
            effective_bytes = Path(metadata["effective_config"]).read_bytes()
            if hashlib.sha256(effective_bytes).hexdigest() != metadata["effective_config_sha256"]:
                result["problems"].append("Effective configuration changed during the run")
            effective = tomllib.loads(effective_bytes.decode("utf-8"))
            result["renderer"] = metadata["renderer"]
            if backend == "rubicon":
                actual = {"workers": 1, "max_in_flight": 16, "queue_capacity": 16} | effective.get("renderer", {})
                if actual != metadata["renderer"] or actual["workers"] != workers:
                    result["problems"].append("Effective renderer configuration does not match its metadata")
                if renderer is not None and actual != renderer:
                    result["problems"].append("Effective renderer configuration does not match the planned variant")
            if warmup["backend"] != backend or warmup["workers"] != workers:
                result["problems"].append("Warmup data belongs to another backend or worker count")
            if warmup["requests"] != metadata["warmup_requests"] or warmup["vus"] != metadata["warmup_vus"]:
                result["problems"].append("Warmup data does not match execution metadata")
            if warmup["minimum_per_worker"] != metadata["warmup_per_worker"]:
                result["problems"].append("Warmup threshold does not match execution metadata")
            if metadata["warmup_requests"]:
                warm_metrics = json.loads((directory / "warmup.k6.json").read_text())["metrics"]
                if warm_metrics["http_reqs"]["count"] != metadata["warmup_requests"] or warm_metrics["correct_responses"]["count"] != metadata["warmup_requests"]:
                    result["problems"].append("Warmup request accounting or correctness failed")
                if not warmup["performed"] or warmup["verified"] is not True or warmup["problems"]:
                    result["problems"].append("Warmup did not complete successfully")
            if metadata["warmup_per_worker"]:
                expected = {str(index): metadata["warmup_per_worker"] for index in range(1, workers + 1)}
                if warmup["required"] is not True or warmup["confirmations"] != expected:
                    result["problems"].append("Not every runtime confirmed the required warmup threshold")
            result["warmup"] = warmup
        except (OSError, ValueError, KeyError, TypeError) as exception:
            result["problems"].append(f"Incomplete execution metadata: {exception}")
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
    def profile_key(run):
        renderer = run.get("renderer") or {}
        return (run["vus"], run["backend"], run.get("workers", 1), renderer.get("max_in_flight"), renderer.get("queue_capacity"), run.get("semi_space_mib", 0), run.get("body_transport", "text" if run["backend"] == "rubicon" else "native"), run.get("http_threads"), run.get("http_threads_actual"))
    keys = sorted({profile_key(run) for run in runs}, key=lambda value: (*value[:3], value[3] or 0, value[4] or 0, value[5], value[6], -1 if value[7] is None else value[7], value[8] or 0))
    for vus, backend, workers, max_in_flight, queue_capacity, semi_space_mib, transport, threads, actual_threads in keys:
        members = [run for run in runs if profile_key(run) == (vus, backend, workers, max_in_flight, queue_capacity, semi_space_mib, transport, threads, actual_threads)]
        observed = [run for run in members if "k6_window_s" in run]
        counts = {name: sum(run["counts"][name] for run in observed) for name in OUTCOMES}
        elapsed = sum(run["k6_window_s"] for run in observed)
        correct = counts["correct"]
        group = {
            "vus": vus, "backend": backend, "runs": len(members),
            "workers": workers, "max_in_flight": max_in_flight, "queue_capacity": queue_capacity,
            "semi_space_mib": semi_space_mib,
            "body_transport": transport,
            "http_threads": threads, "http_threads_actual": actual_threads,
            "total_in_flight": None if max_in_flight is None else workers * max_in_flight,
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
        "schema_version": 2, "completed": manifest["completed"], "all_valid": valid,
        "artifacts_unchanged": manifest["artifacts_unchanged"], "runs": runs, "groups": groups,
        "definitions": {
            "throughput": "Correct response count divided by the sum of k6 execution windows; not the mean of run rates.",
            "latency": "Mean weighted by correct responses. p95 and p99 are retained per run; they are not pooled or averaged.",
            "ttfb": "k6 HTTP waiting time for correct responses, excluding connection establishment.",
            "cpu": "User + system time over each full measurement phase, divided by correct responses. Incomplete CPU totals produce n/a.",
            "ram": "Maximum sampled server-tree RSS per run, in MiB. Shared pages may be counted more than once; brief peaks may be missed.",
            "validity": "Errors and incomplete runs are retained. An invalid or unfinished suite does not establish a performance advantage.",
            "workers": "Rubicon worker count denotes separate V8 runtimes and threads in one process. Node is the current single-process baseline, without cluster workers. No CPU quota is enforced.",
            "warmup": "Warmup requests must pass HTML checks; opted-in Rubicon runs must confirm the configured minimum for every runtime before measurement. This confirms request coverage, not complete JIT stabilization.",
            "semi_space": "Requested V8 semi-space cap per Rubicon runtime in MiB. Zero means the engine default. It is not a total heap, RSS, or process memory cap. Node remains untuned in this experiment.",
            "body_transport": "Rubicon text decodes the Response body before V8-to-Rust conversion; bytes keeps binary data and copies it into Rust-owned memory. Node retains its native adapter. Both Rubicon variants buffer the complete response.",
            "http_threads": "HTTP Tokio worker threads are separate from V8 renderer workers and V8 background threads. Requested 0 clears TOKIO_WORKER_THREADS; the demo reports the effective count from its runtime metrics before load. Each group retains both requested and effective counts. Profiles are scoped to server children; Node receives no override.",
        },
    }
    directory.joinpath("report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    lines = [
        "# Products comparison", "", f"Completed: {manifest['completed']}. All runs valid: {valid}.", "",
        "Configured server artifacts (see manifest); sequential servers, alternating order, fresh server per run.",
        "Fixed VUs with finite iterations. CPU and RAM are observed; no resource caps are applied.", "",
        "| Backend | Workers | HTTP requested | HTTP actual | Body | Semi-space MiB | Active/worker | Total active | VUs | Runs | Valid | Correct/s | Mean ms | TTFB ms | p95 range ms | p99 range ms | CPU ms/correct | RSS peak MiB | Incorrect |",
        "|---|---:|---|---:|---|---:|---:|---:|---:|---:|---|---:|---:|---:|---|---|---:|---:|---:|",
    ]
    for group in groups:
        server = group["resources"].get("server", {})
        peaks = server.get("rss_peak_mib_per_run", [])
        errors = sum(value for name, value in group["counts"].items() if name != "correct")
        lines.append(
            f"| {group['backend']} | {group['workers']} | {'auto' if group['http_threads'] == 0 else group['http_threads'] or '-'} | {group['http_threads_actual'] or '-'} | {group['body_transport']} | {group['semi_space_mib'] or 'default'} | {group['max_in_flight'] or '-'} | {group['total_in_flight'] or '-'} | "
            f"{group['vus']} | {group['runs']} | {group['all_valid']} | "
            f"{display(group['correct_rps'])} | {display(group['mean_ms'])} | {display(group['ttfb_mean_ms'])} | "
            f"{interval(group['p95_ms_per_run'])} | {interval(group['p99_ms_per_run'])} | "
            f"{display(server.get('cpu_ms_per_correct_request'))} | {display(max(peaks) if peaks else None)} | {errors} |"
        )
    lines.extend(["", "Percentile ranges show the separate runs. CPU 100% means one logical CPU; it may exceed 100%.", ""])
    lines.extend([manifest.get("method", ""), "", report["definitions"]["workers"], "", report["definitions"]["warmup"], ""])
    lines.extend([report["definitions"]["semi_space"], ""])
    lines.extend([report["definitions"]["body_transport"], ""])
    lines.extend([report["definitions"]["http_threads"], ""])
    for run in runs:
        if run["problems"]:
            lines.append(f"- {Path(run['directory']).name}: {'; '.join(run['problems'])}")
    directory.joinpath("report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return report
