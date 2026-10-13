import argparse
from contextlib import nullcontext
from pathlib import Path
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
    vus = 1 if phase == "Warmup" else args.vus
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
    parser.add_argument("--requests", type=int, default=100)
    parser.add_argument("--sample-ms", type=int, default=100)
    parser.add_argument("--process-scan-ms", type=int, default=1000)
    parser.add_argument("--vus", type=int, default=1)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--capture-k6-output", action="store_true")

    args = parser.parse_args(argv)

    if args.warmup < 0:
        parser.error("--warmup must be zero or greater")

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
    url = config["server"]["origin"].rstrip("/") + "/products"

    k6 = shutil.which("k6") or r"C:\Program Files\k6\k6.exe"

    if not SCRIPT.is_file():
        raise RuntimeError(f"Missing k6 script: {SCRIPT}")

    run_dir = (
        args.output_dir.resolve() if args.output_dir
        else ROOT / "benchmarks/results" / f"{args.backend}-{uuid.uuid4().hex}"
    )
    run_dir.mkdir(parents=True, exist_ok=False)

    output_path = run_dir / "server.stdout.log"
    error_path = run_dir / "server.stderr.log"

    print(f"Backend: {args.backend}")
    print(f"Logs: {run_dir}")

    command = [
        str(RB),
        "serve",
        "--config",
        str(config_path),
        "--backend",
        args.backend,
        "--managed",
    ]

    with output_path.open("wb") as output, error_path.open("wb") as errors:
        process = subprocess.Popen(
            command,
            cwd=ROOT,
            stdin=subprocess.PIPE,
            stdout=output,
            stderr=errors,
        )

        try:
            wait_for_server(process, output_path, args.backend)
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
                    break
        finally:
            process.stdin.close()
            server_code = process.wait(timeout=15)

    if server_code != 0:
        raise RuntimeError(f"Server shutdown failed; check {error_path}")

    return exit_code


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.TimeoutExpired, psutil.Error) as error:
        print(f"Error: {error}", file=sys.stderr)
        raise SystemExit(1)
