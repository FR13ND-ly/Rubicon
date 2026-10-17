import argparse
from contextlib import nullcontext
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import tomllib
import uuid

try:
    import psutil
except ModuleNotFoundError:
    raise SystemExit(
        "Missing psutil. Use target/benchmark-python/Scripts/python.exe, "
        "or install benchmarks/requirements.txt in your Python environment."
    )

from resource_monitor import ResourceMonitor, completed_cpu_seconds, find_server


ROOT = Path(__file__).resolve().parent.parent
RB = ROOT / "target/debug/rb.exe"
SCRIPT = ROOT / "benchmarks/k6/products.js"


def renderer_settings(config, workers=None, max_in_flight=None, queue_capacity=None):
    settings = {"workers": 1, "max_in_flight": 16, "queue_capacity": 16} | config.get("renderer", {})
    if workers is not None:
        settings["workers"] = workers
    if max_in_flight is not None:
        settings["max_in_flight"] = max_in_flight
    if queue_capacity is not None:
        settings["queue_capacity"] = queue_capacity
    for key, value in settings.items():
        if isinstance(value, bool) or not isinstance(value, int) or value < 1:
            raise ValueError(f"renderer.{key} must be a positive integer")
    return settings


def write_effective_config(destination, source_path, config, renderer):
    settings = {section: dict(values) for section, values in config.items()}
    settings["renderer"] = dict(renderer)
    for backend in ("rubicon", "node"):
        for key in ("executable", "server_entry", "browser_dir"):
            value = settings[backend][key]
            if key == "executable" and Path(value).name == value:
                continue
            settings[backend][key] = (source_path.parent / value).resolve().as_posix()
    lines = []
    for section, values in settings.items():
        lines.append(f"[{section}]")
        lines.extend(f"{key} = {json.dumps(value, ensure_ascii=False)}" for key, value in values.items())
        lines.append("")
    destination.write_text("\n".join(lines), encoding="utf-8")


def warmup_coverage(log_path, workers, minimum):
    confirmations = {}
    problems = []
    pattern = re.compile(r"RUBICON_DEMO_WARMUP_READY runtime_id=(\d+) completed=(\d+)")
    for line in log_path.read_text(encoding="utf-8", errors="replace").splitlines():
        if not line.startswith("RUBICON_DEMO_WARMUP_READY"):
            continue
        match = pattern.fullmatch(line)
        if match is None:
            problems.append(f"Malformed warmup confirmation: {line}")
            continue
        runtime_id, count = map(int, match.groups())
        if runtime_id in confirmations:
            problems.append(f"Duplicate warmup confirmation for runtime {runtime_id}")
        confirmations[runtime_id] = count
        if not 1 <= runtime_id <= workers:
            problems.append(f"Unexpected warmup runtime {runtime_id}")
        if count != minimum:
            problems.append(f"Runtime {runtime_id} confirmed {count} renders instead of {minimum}")
    missing = sorted(set(range(1, workers + 1)) - set(confirmations))
    if missing:
        problems.append(f"Missing warmup confirmations for runtimes {missing}")
    return {"verified": not problems, "confirmations": confirmations, "problems": problems}


def confirmed_http_threads(log_path, requested):
    markers = [line for line in log_path.read_text(encoding="utf-8").splitlines()
               if line.startswith("RUBICON_DEMO_HTTP_THREADS")]
    match = re.fullmatch(r"RUBICON_DEMO_HTTP_THREADS=([1-9]\d*)", markers[0]) if len(markers) == 1 else None
    if match is None:
        raise RuntimeError("The server did not confirm exactly one valid HTTP thread count")
    actual = int(match.group(1))
    if requested and actual != requested:
        raise RuntimeError(f"Requested {requested} HTTP threads, but the server confirmed {actual}")
    return actual


def wait_for_server(process, output_path, backend):
    marker = (
        "Rubicon is running at "
        if backend == "rubicon"
        else "Node Express server listening on "
    )

    deadline = time.monotonic() + 20

    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError("Server exited before startup; check the logs.")

        output = output_path.read_text(encoding="utf-8", errors="replace")

        if marker in output:
            return

        time.sleep(0.1)

    raise RuntimeError("Server startup exceeded 20 seconds.")


def run_k6(k6, url, requests, phase, server, launcher, args, run_dir):
    vus = args.warmup_vus if phase == "Warmup" else args.vus
    print(f"\n{phase}: {requests} requests, vus={vus}", flush=True)

    max_duration_s = requests * 10 + 15

    command = [
        k6,
        "run",
        "--no-usage-report",
        "--summary-export",
        str(run_dir / f"{phase.lower()}.k6.json"),
        "--summary-trend-stats",
        "avg,min,med,max,p(90),p(95),p(99)",
        "--vus",
        str(vus),
        "--iterations",
        str(requests),
        "--duration",
        f"{max_duration_s}s",
        "--env",
        f"TARGET_URL={url}",
        str(SCRIPT),
    ]
    output = (
        (run_dir / f"{phase.lower()}.k6.log").open("wb")
        if args.capture_k6_output else nullcontext(None)
    )
    with output as log:
        monitor = ResourceMonitor(server, launcher, args.sample_ms, args.process_scan_ms)
        result = subprocess.Popen(
            command, cwd=ROOT, stdout=log,
            stderr=subprocess.STDOUT if log is not None else None,
        )
        error = None
        try:
            monitor.add_generator(result.pid)
            deadline = time.monotonic() + max_duration_s + 45
            while True:
                monitor.sample()
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise subprocess.TimeoutExpired(result.args, max_duration_s + 45)
                try:
                    return result.wait(timeout=min(args.sample_ms / 1000, remaining))
                except subprocess.TimeoutExpired:
                    continue
        except BaseException as exception:
            error = f"{type(exception).__name__}: {exception}"
            raise
        finally:
            if result.poll() is None:
                result.kill()
                result.wait(timeout=10)
            if "k6" in monitor.groups:
                monitor.groups["k6"].finalize_root(completed_cpu_seconds(result))
            monitor.sample(force_discovery=True)
            monitor.save(
                run_dir / f"{phase.lower()}.resources.json",
                phase, args.backend, requests, result.returncode, error, vus=vus,
            )


def main(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--backend",
        choices=["rubicon", "node"],
        default="rubicon",
    )
    parser.add_argument(
        "--config",
        type=Path,
        default=ROOT / "examples/angular-demo/rubicon.toml",
    )
    parser.add_argument("--warmup", type=int, default=20)
    parser.add_argument("--warmup-vus", type=int, default=1)
    parser.add_argument("--warmup-per-worker", type=int, default=0)
    parser.add_argument("--workers", type=int)
    parser.add_argument("--max-in-flight", type=int)
    parser.add_argument("--queue-capacity", type=int)
    parser.add_argument("--http-threads", type=int,
                        help="Rubicon HTTP Tokio threads; 0 clears the override and measures the Tokio default")
    parser.add_argument("--body-transport", choices=["text", "bytes", "stream"],
                        help="Rubicon demo body transport; omitted keeps the demo default")
    parser.add_argument("--semi-space-mib", type=int, choices=[0, 1, 2, 4, 8, 16, 32, 64], default=0,
                        help="Rubicon V8 semi-space limit per runtime; 0 keeps engine defaults")
    parser.add_argument("--requests", type=int, default=100)
    parser.add_argument("--sample-ms", type=int, default=100)
    parser.add_argument("--process-scan-ms", type=int, default=1000)
    parser.add_argument("--vus", type=int, default=1)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--capture-k6-output", action="store_true")

    args = parser.parse_args(argv)

    if args.warmup < 0:
        parser.error("--warmup must be zero or greater")
    if args.warmup_vus < 1 or (args.warmup and args.warmup < args.warmup_vus):
        parser.error("--warmup-vus must be positive and no larger than a nonzero --warmup")
    if args.warmup_per_worker < 0:
        parser.error("--warmup-per-worker must be nonnegative")
    if args.backend == "node" and args.workers not in (None, 1):
        parser.error("The current Node baseline has one process; --workers must be 1")
    if args.backend == "node" and args.semi_space_mib:
        parser.error("--semi-space-mib currently applies only to the Rubicon demo")
    if args.backend == "node" and args.body_transport:
        parser.error("--body-transport applies only to the Rubicon demo")
    if args.http_threads is not None and (args.http_threads < 0 or args.backend == "node"):
        parser.error("--http-threads must be nonnegative and applies only to Rubicon")

    if args.requests < 1:
        parser.error("--requests must be at least 1")

    if args.sample_ms < 10:
        parser.error("--sample-ms must be at least 10")

    if args.process_scan_ms < args.sample_ms:
        parser.error("--process-scan-ms must be at least --sample-ms")

    if args.vus < 1:
        parser.error("--vus must be at least 1")

    if args.requests < args.vus:
        parser.error("--requests must be at least --vus")

    config_path = args.config.resolve()
    config = tomllib.loads(config_path.read_text(encoding="utf-8"))
    renderer = renderer_settings(config, args.workers, args.max_in_flight, args.queue_capacity)
    workers = renderer["workers"] if args.backend == "rubicon" else 1
    minimum = args.warmup_per_worker if args.backend == "rubicon" else 0
    if minimum and args.warmup < workers * minimum:
        parser.error("--warmup must cover --warmup-per-worker for every worker")
    url = config["server"]["origin"].rstrip("/") + "/products"

    k6 = shutil.which("k6") or r"C:\Program Files\k6\k6.exe"

    if not SCRIPT.is_file():
        raise RuntimeError(f"Missing k6 script: {SCRIPT}")

    run_dir = (
        args.output_dir.resolve() if args.output_dir
        else ROOT / "benchmarks/results" / f"{args.backend}-{uuid.uuid4().hex}"
    )
    run_dir.mkdir(parents=True, exist_ok=False)

    effective_path = config_path
    if args.backend == "rubicon" and any(value is not None for value in (args.workers, args.max_in_flight, args.queue_capacity)):
        effective_path = run_dir / "effective.toml"
        write_effective_config(effective_path, config_path, config, renderer)
    metadata = {
        "schema_version": 1, "backend": args.backend, "workers": workers,
        "semi_space_mib": args.semi_space_mib,
        "body_transport": args.body_transport or ("text" if args.backend == "rubicon" else "native"),
        "http_threads": args.http_threads, "http_threads_actual": None,
        "renderer": renderer if args.backend == "rubicon" else None,
        "source_config": str(config_path), "effective_config": str(effective_path),
        "effective_config_sha256": hashlib.sha256(effective_path.read_bytes()).hexdigest(),
        "requests": args.requests, "vus": args.vus,
        "warmup_requests": args.warmup, "warmup_vus": args.warmup_vus,
        "warmup_per_worker": minimum,
    }
    (run_dir / "run.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    coverage = {
        "backend": args.backend, "workers": workers, "required": minimum > 0,
        "performed": args.warmup > 0, "requests": args.warmup,
        "vus": args.warmup_vus, "minimum_per_worker": minimum,
        "verified": False, "confirmations": {}, "problems": [],
    }

    output_path = run_dir / "server.stdout.log"
    error_path = run_dir / "server.stderr.log"

    print(f"Backend: {args.backend}; workers={workers}; renderer={metadata['renderer']}")
    print(f"Logs: {run_dir}")

    command = [
        str(RB),
        "serve",
        "--config",
        str(effective_path),
        "--backend",
        args.backend,
        "--managed",
    ]

    environment = os.environ.copy()
    environment.pop("RUBICON_DEMO_WARMUP_REQUESTS", None)
    environment.pop("RUBICON_DEMO_V8_SEMI_SPACE_MIB", None)
    environment.pop("RUBICON_DEMO_BODY_TRANSPORT", None)
    environment.pop("TOKIO_WORKER_THREADS", None)
    if minimum:
        environment["RUBICON_DEMO_WARMUP_REQUESTS"] = str(minimum)
    if args.semi_space_mib:
        environment["RUBICON_DEMO_V8_SEMI_SPACE_MIB"] = str(args.semi_space_mib)
    if args.body_transport:
        environment["RUBICON_DEMO_BODY_TRANSPORT"] = args.body_transport
    if args.http_threads:
        environment["TOKIO_WORKER_THREADS"] = str(args.http_threads)

    with output_path.open("wb") as output, error_path.open("wb") as errors:
        process = subprocess.Popen(
            command,
            cwd=ROOT,
            stdin=subprocess.PIPE,
            stdout=output,
            stderr=errors,
            env=environment,
        )

        try:
            wait_for_server(process, output_path, args.backend)
            if args.http_threads is not None:
                metadata["http_threads_actual"] = confirmed_http_threads(error_path, args.http_threads)
                (run_dir / "run.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
                print(f"HTTP Tokio threads: requested={args.http_threads or 'auto'}, actual={metadata['http_threads_actual']}", flush=True)
            if args.semi_space_mib:
                expected = f"RUBICON_DEMO_V8_SEMI_SPACE_MIB={args.semi_space_mib}"
                confirmations = error_path.read_text(encoding="utf-8").splitlines().count(expected)
                if confirmations != 1:
                    raise RuntimeError("The server did not confirm the requested V8 memory profile")
            if args.body_transport:
                expected = f"RUBICON_DEMO_BODY_TRANSPORT={args.body_transport}"
                if error_path.read_text(encoding="utf-8").splitlines().count(expected) != 1:
                    raise RuntimeError("The server did not confirm the requested body transport")
            server = find_server(
                process.pid, config[args.backend]["executable"], config_path.parent
            )
            launcher = psutil.Process(process.pid)

            exit_code = 0

            for phase, requests in [
                ("Warmup", args.warmup),
                ("Measurement", args.requests),
            ]:
                if requests == 0:
                    continue

                exit_code = run_k6(
                    k6, url, requests, phase, server, launcher, args, run_dir
                )

                if exit_code != 0:
                    if phase == "Warmup":
                        coverage["problems"].append(f"Warmup k6 exited with code {exit_code}")
                    break
                if phase == "Warmup":
                    coverage["verified"] = True
                    if minimum:
                        coverage.update(warmup_coverage(error_path, workers, minimum))
                    (run_dir / "warmup.json").write_text(json.dumps(coverage, indent=2) + "\n", encoding="utf-8")
                    if not coverage["verified"]:
                        raise RuntimeError("Warmup is incomplete: " + "; ".join(coverage["problems"]))
                    print(f"Warmup verified: {coverage['confirmations'] or 'correctness checks passed; per-runtime probe disabled'}", flush=True)
        finally:
            process.stdin.close()
            server_code = process.wait(timeout=15)
            if minimum and coverage["verified"]:
                coverage.update(warmup_coverage(error_path, workers, minimum))
            (run_dir / "warmup.json").write_text(json.dumps(coverage, indent=2) + "\n", encoding="utf-8")

    if server_code != 0:
        raise RuntimeError(f"Server shutdown failed; check {error_path}")
    if minimum and not coverage["verified"]:
        raise RuntimeError("Runtime warmup verification failed: " + "; ".join(coverage["problems"]))

    return exit_code


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.TimeoutExpired, psutil.Error) as error:
        print(f"Error: {error}", file=sys.stderr)
        raise SystemExit(1)
