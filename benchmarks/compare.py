import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tomllib
import uuid

import run
from reporting import display, interval, measurement, write_report


ROOT = Path(__file__).resolve().parent.parent


def planned_runs(vus_levels, rounds, workers_levels=None, total_in_flight=16, queue_capacity=16, semi_spaces=None, body_transports=None, http_threads=None, backends=None):
    plan = []
    active_levels = [total_in_flight] if isinstance(total_in_flight, int) else total_in_flight
    variants = [
        {"backend": "rubicon", "workers": workers, "semi_space_mib": semi, "body_transport": transport, "http_threads": threads, "renderer": {
            "workers": workers, "max_in_flight": total // workers,
            "queue_capacity": queue_capacity,
        }}
        for workers in ([1] if workers_levels is None else workers_levels)
        for semi in ([0] if semi_spaces is None else semi_spaces)
        for total in active_levels
        for transport in (["text"] if body_transports is None else body_transports)
        for threads in ([None] if http_threads is None else http_threads)
    ] + [{"backend": "node", "workers": 1, "semi_space_mib": 0, "body_transport": "native", "http_threads": None, "renderer": None}]
    if backends is not None:
        variants = [item for item in variants if item["backend"] in backends]
    for round_index in range(rounds):
        levels = vus_levels if round_index % 2 == 0 else list(reversed(vus_levels))
        for vus in levels:
            shift = (round_index + vus_levels.index(vus)) % len(variants)
            ordered = variants[shift:] + variants[:shift]
            for variant in ordered:
                plan.append({"round": round_index + 1, "vus": vus} | variant)
    return plan


def executable(value, base):
    path = Path(value)
    if path.name == value:
        found = shutil.which(value)
        if not found:
            raise FileNotFoundError(f"Executable not found: {value}")
        return Path(found)
    path = (base / path).resolve()
    if os.name == "nt" and not path.suffix:
        path = path.with_suffix(".exe")
    return path


def snapshot(config_path, config):
    files = {config_path, ROOT / "Cargo.toml", ROOT / "Cargo.lock", run.RB, run.SCRIPT}
    for directory in [ROOT / "crates", ROOT / "examples/angular-demo/src", ROOT / "examples/angular-demo/rust-server"]:
        for parent, directories, names in os.walk(directory):
            directories[:] = [name for name in directories if name not in {"target", "node_modules", ".git", "__pycache__"}]
            files.update(Path(parent) / name for name in names if Path(name).suffix in {".rs", ".js", ".mjs", ".ts", ".html", ".css", ".toml"})
    files.update(path for path in (ROOT / "benchmarks").glob("*.py"))
    files.add(ROOT / "benchmarks/requirements.txt")
    for name in ["package.json", "package-lock.json", "angular.json", "tsconfig.json", "tsconfig.app.json"]:
        files.add(ROOT / "examples/angular-demo" / name)
    for backend in ["rubicon", "node"]:
        settings = config[backend]
        files.add(executable(settings["executable"], config_path.parent))
        entry = (config_path.parent / settings["server_entry"]).resolve()
        files.add(entry)
        files.update(path for path in entry.parent.rglob("*") if path.is_file())
        browser = (config_path.parent / settings["browser_dir"]).resolve()
        if not browser.is_dir():
            raise FileNotFoundError(f"Browser assets not found: {browser}")
        files.update(path for path in browser.rglob("*") if path.is_file())
    hashes = {}
    for path in sorted(files):
        digest = hashlib.sha256()
        with path.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
        hashes[str(path)] = digest.hexdigest()
    return hashes


def output(command):
    try:
        result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=10)
        return result.stdout.strip() if result.returncode == 0 else None
    except (OSError, subprocess.TimeoutExpired):
        return None


def main(argv=None):
    parser = argparse.ArgumentParser(description="Compare products SSR with sequential Rubicon and Node runs")
    parser.add_argument("--config", type=Path, default=ROOT / "examples/angular-demo/rubicon.toml")
    parser.add_argument("--backends", choices=["rubicon", "node"], nargs="+", default=["rubicon", "node"])
    parser.add_argument("--http-threads", type=int, nargs="+",
                        help="Rubicon HTTP Tokio thread profiles; 0 measures the unmodified Tokio default")
    parser.add_argument("--rounds", type=int, default=2)
    parser.add_argument("--vus", type=int, nargs="+", default=[1, 4, 16])
    parser.add_argument("--workers", type=int, nargs="+", default=[1, 2, 4])
    parser.add_argument("--total-in-flight", type=int, nargs="+", default=[16])
    parser.add_argument("--body-transports", choices=["text", "bytes", "stream"], nargs="+", default=["text"])
    parser.add_argument("--queue-capacity", type=int)
    parser.add_argument("--semi-spaces", type=int, nargs="+", choices=[0, 1, 2, 4, 8, 16, 32, 64], default=[0],
                        help="Rubicon V8 semi-space profiles in MiB; 0 keeps engine defaults")
    parser.add_argument("--warmup", type=int, default=100)
    parser.add_argument("--warmup-vus", type=int, default=16)
    parser.add_argument("--warmup-per-worker", type=int, default=20)
    parser.add_argument("--requests", type=int, default=1000)
    parser.add_argument("--sample-ms", type=int, default=100)
    parser.add_argument("--process-scan-ms", type=int, default=1000)
    args = parser.parse_args(argv)
    if args.rounds < 1 or args.warmup < 0:
        parser.error("--rounds must be positive and --warmup must be nonnegative")
    if len(set(args.backends)) != len(args.backends):
        parser.error("--backends must contain distinct values")
    if args.http_threads is not None and (
        "rubicon" not in args.backends or len(set(args.http_threads)) != len(args.http_threads)
        or any(value < 0 for value in args.http_threads)
    ):
        parser.error("--http-threads requires Rubicon and distinct nonnegative values")
    if any(vus < 1 for vus in args.vus) or len(set(args.vus)) != len(args.vus):
        parser.error("--vus must contain distinct positive integers")
    if any(workers < 1 for workers in args.workers) or len(set(args.workers)) != len(args.workers):
        parser.error("--workers must contain distinct positive integers")
    if len(set(args.semi_spaces)) != len(args.semi_spaces):
        parser.error("--semi-spaces must contain distinct values")
    if len(set(args.body_transports)) != len(args.body_transports):
        parser.error("--body-transports must contain distinct values")
    if len(set(args.total_in_flight)) != len(args.total_in_flight) or any(
        total < 1 or total % workers for total in args.total_in_flight for workers in args.workers
    ):
        parser.error("--total-in-flight must contain distinct positive values divisible by every worker count")
    if args.warmup_vus < 1 or (args.warmup and args.warmup < args.warmup_vus):
        parser.error("--warmup-vus must be positive and no larger than a nonzero --warmup")
    if args.warmup_per_worker < 0 or args.warmup < max(args.workers) * args.warmup_per_worker:
        parser.error("--warmup must cover a nonnegative --warmup-per-worker for every runtime")
    if args.requests < max(args.vus):
        parser.error("--requests must be at least the largest VU count")
    if args.sample_ms < 10 or args.process_scan_ms < args.sample_ms:
        parser.error("Sampling must be at least 10 ms; process scans must not be more frequent than samples")

    config_path = args.config.resolve()
    config = tomllib.loads(config_path.read_text(encoding="utf-8"))
    configured_renderer = run.renderer_settings(config)
    queue_capacity = args.queue_capacity if args.queue_capacity is not None else configured_renderer["queue_capacity"]
    if queue_capacity < 1:
        parser.error("--queue-capacity must be positive")
    configured_renderer["queue_capacity"] = queue_capacity
    frozen = snapshot(config_path, config)
    plan = planned_runs(args.vus, args.rounds, args.workers, args.total_in_flight, queue_capacity, args.semi_spaces, args.body_transports, args.http_threads, args.backends)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
    directory = ROOT / "benchmarks/results" / f"comparison-{stamp}-{uuid.uuid4().hex[:8]}"
    directory.mkdir(parents=True)
    manifest = {
        "schema_version": 2, "started_utc": datetime.now(timezone.utc).isoformat(),
        "completed": False, "artifacts_unchanged": None,
        "config": str(config_path), "parameters": vars(args) | {"config": str(config_path)},
        "renderer": configured_renderer,
        "total_in_flight": args.total_in_flight,
        "run_order": plan, "input_sha256": frozen,
        "git_head": output(["git", "rev-parse", "HEAD"]),
        "git_status": output(["git", "status", "--short"]),
        "environment": {
            "platform": platform.platform(), "processor": platform.processor(),
            "python": sys.version, "psutil": run.psutil.__version__,
            "node": output([str(executable(config["node"]["executable"], config_path.parent)), "--version"]),
            "k6": output([shutil.which("k6") or r"C:\Program Files\k6\k6.exe", "version"]),
            "rustc": output(["rustc", "--version"]),
            "logical_cpus": run.psutil.cpu_count(), "physical_cpus": run.psutil.cpu_count(logical=False),
            "ram_bytes": run.psutil.virtual_memory().total,
            "windows_timer_resolution_override": os.environ.get("RUBICON_WINDOWS_TIMER_RESOLUTION"),
            "inherited_tokio_worker_threads": os.environ.get("TOKIO_WORKER_THREADS"),
        },
        "method": f"Fresh server per run; {args.warmup} warmup requests at {args.warmup_vus} VUs; at least {args.warmup_per_worker} completed products renders per Rubicon runtime confirmed before measurement; fixed shared iterations; sequential servers; variant order rotated and VU order alternated; total active Rubicon render profiles {args.total_in_flight} and {queue_capacity} global queued renders; V8 semi-space profiles {args.semi_spaces} MiB (0 keeps engine defaults); body transport profiles {args.body_transports}; HTTP Tokio thread profiles {args.http_threads} (0 clears TOKIO_WORKER_THREADS; effective count confirmed before load); backends {args.backends}; Node, when selected, single process with native transport; no CPU/total RSS caps.",
    }
    runs = []

    def save():
        (directory / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        return write_report(directory, runs, manifest)

    save()
    print(f"Comparison: {directory}", flush=True)
    print(f"{len(plan)} runs; {args.requests} measured requests per run", flush=True)
    try:
        for sequence, item in enumerate(plan, 1):
            backend, vus, workers = item["backend"], item["vus"], item["workers"]
            semi = item["semi_space_mib"]
            transport = item["body_transport"]
            threads = item["http_threads"]
            active = item["renderer"]["max_in_flight"] * workers if item["renderer"] else 0
            child = directory / f"{sequence:02d}-round{item['round']}-vu{vus}-{backend}-w{workers}-a{active}-s{semi}-{transport}-h{threads if threads is not None else 'default'}"
            print(f"\n[{sequence}/{len(plan)}] round={item['round']} backend={backend} workers={workers} active={active} semi_space_mib={semi} body={transport} http_threads={threads} vus={vus}", flush=True)
            error = None
            code = None
            try:
                command = [
                    "--backend", backend, "--config", str(config_path),
                    "--warmup", str(args.warmup), "--requests", str(args.requests), "--vus", str(vus),
                    "--warmup-vus", str(args.warmup_vus),
                    "--warmup-per-worker", str(args.warmup_per_worker),
                    "--sample-ms", str(args.sample_ms), "--process-scan-ms", str(args.process_scan_ms),
                    "--output-dir", str(child), "--capture-k6-output",
                    "--semi-space-mib", str(semi),
                ]
                if backend == "rubicon":
                    command.extend([
                        "--body-transport", transport,
                        "--workers", str(workers),
                        "--max-in-flight", str(item["renderer"]["max_in_flight"]),
                        "--queue-capacity", str(item["renderer"]["queue_capacity"]),
                    ])
                    if threads is not None:
                        command.extend(["--http-threads", str(threads)])
                code = run.main(command)
            except (OSError, RuntimeError, subprocess.TimeoutExpired, run.psutil.Error) as exception:
                error = f"{type(exception).__name__}: {exception}"
                print(error, file=sys.stderr, flush=True)
            except KeyboardInterrupt:
                interrupted = measurement(child, backend, vus, args.requests, None, "Interrupted by user", workers, item["renderer"], semi, transport, threads)
                interrupted.update(sequence=sequence, round=item["round"])
                runs.append(interrupted)
                raise
            result = measurement(child, backend, vus, args.requests, code, error, workers, item["renderer"], semi, transport, threads)
            result.update(sequence=sequence, round=item["round"])
            runs.append(result)
            save()
        manifest["completed"] = True
    finally:
        manifest["finished_utc"] = datetime.now(timezone.utc).isoformat()
        try:
            current = snapshot(config_path, config)
            manifest["artifacts_unchanged"] = current == frozen
            manifest["changed_inputs"] = [path for path in sorted(set(current) | set(frozen)) if current.get(path) != frozen.get(path)]
        except OSError as exception:
            manifest["artifacts_unchanged"] = False
            manifest["snapshot_error"] = str(exception)
        report = save()

    print("\nBackend  Workers  Active  HTTP actual  Body    Semi MiB  VUs  Valid  Correct/s  Mean ms  p95 ms/run  CPU ms/correct  RSS peak MiB")
    for group in report["groups"]:
        server = group["resources"].get("server", {})
        peaks = server.get("rss_peak_mib_per_run", [])
        print(
            f"{group['backend']:8} {group['workers']:7} {str(group['total_in_flight'] or '-'):>6} {str(group['http_threads_actual'] or '-'):>11} {group['body_transport']:7} {group['semi_space_mib']:8} {group['vus']:3} {str(group['all_valid']):5} "
            f"{display(group['correct_rps']):>10} {display(group['mean_ms']):>8} "
            f"{interval(group['p95_ms_per_run']):>12} {display(server.get('cpu_ms_per_correct_request')):>14} "
            f"{display(max(peaks) if peaks else None):>12}"
        )
    print(f"\nReport: {directory / 'report.md'}")
    if not report["all_valid"]:
        print("Comparison is invalid or incomplete; see report.json and individual logs.", file=sys.stderr)
    return 0 if report["all_valid"] else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        print(f"Error: {error}", file=sys.stderr)
        raise SystemExit(1)
