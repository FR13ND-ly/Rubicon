import argparse
from pathlib import Path
import shutil
import subprocess
import sys
import time
import tomllib
import uuid


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


def main():
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
    args = parser.parse_args()

    config_path = args.config.resolve()
    config = tomllib.loads(config_path.read_text(encoding="utf-8"))
    url = config["server"]["origin"].rstrip("/") + "/products"

    k6 = shutil.which("k6") or r"C:\Program Files\k6\k6.exe"

    if not SCRIPT.is_file():
        raise RuntimeError(f"Missing k6 script: {SCRIPT}")

    run_dir = ROOT / "benchmarks/results" / f"{args.backend}-{uuid.uuid4().hex}"
    run_dir.mkdir(parents=True)

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

            result = subprocess.run(
                [
                    k6,
                    "run",
                    "--no-usage-report",
                    "--env",
                    f"TARGET_URL={url}",
                    str(SCRIPT),
                ],
                cwd=ROOT,
                timeout=30,
            )
        finally:
            process.stdin.close()
            server_code = process.wait(timeout=15)

    if server_code != 0:
        raise RuntimeError(f"Server shutdown failed; check {error_path}")

    return result.returncode


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
        print(f"Error: {error}", file=sys.stderr)
        raise SystemExit(1)