import argparse
import csv
import ctypes
import hashlib
import json
import math
import os
import platform
import re
import shutil
import socket
import statistics
import subprocess
import sys
import time
from datetime import datetime
from contextlib import contextmanager
from html import unescape
from pathlib import Path
from urllib.request import ProxyHandler, build_opener


ROOT = Path(__file__).resolve().parents[1]
DEMO = ROOT / "examples/angular-demo"
MEASURE = ROOT / "benchmarks/measure-home.py"
HTTP = build_opener(ProxyHandler({}))
PRODUCTS = [
    {"id": "keyboard", "name": "Tastatură"},
    {"id": "mouse", "name": "Mouse"},
    {"id": "monitor", "name": "Monitor"},
]
PRODUCTS_STATE_KEY = "rubicon-demo:products:v1"


@contextmanager
def keep_system_awake():
    # A thread-scoped request; it neither changes the power plan nor keeps the
    # display on. Windows removes it when this thread exits, even after a crash.
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.SetThreadExecutionState.argtypes = [ctypes.c_uint32]
    kernel.SetThreadExecutionState.restype = ctypes.c_uint32
    previous = kernel.SetThreadExecutionState(0x80000001)
    if not previous:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        if not kernel.SetThreadExecutionState(previous | 0x80000001):
            raise ctypes.WinError(ctypes.get_last_error())
        yield
    finally:
        if not kernel.SetThreadExecutionState(previous):
            raise ctypes.WinError(ctypes.get_last_error())


def power_status():
    class Status(ctypes.Structure):
        _fields_ = [
            ("ac", ctypes.c_ubyte), ("battery_flags", ctypes.c_ubyte),
            ("battery_percent", ctypes.c_ubyte), ("battery_saver", ctypes.c_ubyte),
            ("seconds_remaining", ctypes.c_uint32), ("full_seconds", ctypes.c_uint32),
        ]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetSystemPowerStatus.argtypes = [ctypes.POINTER(Status)]
    kernel.GetSystemPowerStatus.restype = ctypes.c_int
    status = Status()
    if not kernel.GetSystemPowerStatus(ctypes.byref(status)):
        raise ctypes.WinError(ctypes.get_last_error())
    return {"ac_connected": status.ac, "battery_percent": status.battery_percent,
            "battery_saver": status.battery_saver}


def suspension_clock_ms():
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetTickCount64.restype = ctypes.c_uint64
    kernel.QueryUnbiasedInterruptTime.argtypes = [ctypes.POINTER(ctypes.c_uint64)]
    kernel.QueryUnbiasedInterruptTime.restype = ctypes.c_int
    awake_ticks = ctypes.c_uint64()
    if not kernel.QueryUnbiasedInterruptTime(ctypes.byref(awake_ticks)):
        raise ctypes.WinError(ctypes.get_last_error())
    return kernel.GetTickCount64() - awake_ticks.value / 10000


def process_timer_policy(process):
    class State(ctypes.Structure):
        _fields_ = [("version", ctypes.c_uint32), ("control", ctypes.c_uint32),
                    ("state", ctypes.c_uint32)]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.GetProcessInformation.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32]
    kernel.GetProcessInformation.restype = ctypes.c_int
    state = State(1, 0, 0)
    if not kernel.GetProcessInformation(process._handle, 4, ctypes.byref(state), ctypes.sizeof(state)):
        raise ctypes.WinError(ctypes.get_last_error())
    return {"control_mask": state.control, "state_mask": state.state,
            "timer_resolution_explicitly_honored": bool(state.control & 4) and not bool(state.state & 4)}


def ensure_port_free(port):
    with socket.socket() as probe:
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_EXCLUSIVEADDRUSE, 1)

        try:
            probe.bind(("127.0.0.1", port))
        except OSError as error:
            raise RuntimeError(
                f"Port {port} is unavailable. Stop its server first."
            ) from error


def wait_until_ready(process, port, log_path):
    deadline = time.monotonic() + 30

    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"Server exited. Inspect {log_path}")

        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                if process.poll() is None:
                    return
        except OSError:
            pass

        time.sleep(0.1)

    raise RuntimeError(f"Server startup timed out. Inspect {log_path}")


def validate_page(url, path):
    with HTTP.open(url, timeout=5) as response:
        if (
            response.status != 200
            or response.headers.get_content_type() != "text/html"
        ):
            raise RuntimeError(f"Unexpected HTML response from {url}")

        html = response.read().decode("utf-8")

    states = [
        json.loads(value)
        for value in re.findall(
            r'<script\b[^>]*type=[\"\']application/json[\"\'][^>]*>(.*?)</script>',
            html,
            flags=re.DOTALL | re.IGNORECASE,
        )
    ]
    if any(not isinstance(state, dict) for state in states):
        raise RuntimeError(f"Unexpected JSON state at {url}")

    if path == "/products":
        component = re.search(r'<app-products\b[^>]*>(.*?)</app-products>', html, re.DOTALL)
        rendered_products = [] if component is None else [
            unescape(re.sub(r'<[^>]+>', '', item)).strip()
            for item in re.findall(r'<li\b[^>]*>(.*?)</li>', component[1], re.DOTALL)
        ]
        if (
            rendered_products != [product["name"] for product in PRODUCTS]
            or not any(state.get(PRODUCTS_STATE_KEY) == PRODUCTS for state in states)
        ):
            raise RuntimeError(f"Products HTML or TransferState is incorrect at {url}")

        api_url = url.removesuffix("/products") + "/api/products"
        with HTTP.open(api_url, timeout=5) as response:
            if (
                response.status != 200
                or response.headers.get_content_type() != "application/json"
                or json.load(response) != PRODUCTS
            ):
                raise RuntimeError(f"Products API is incorrect at {api_url}")
    else:
        counter = re.search(
            r'<span[^>]*id="counter-value"[^>]*>\s*0\s*</span>', html,
        )
        if "Angular renders this page on the server." not in html or not counter:
            raise RuntimeError(f"Expected Home content is missing at {url}")
        if any(PRODUCTS_STATE_KEY in state for state in states):
            raise RuntimeError(f"Products TransferState leaked into Home at {url}")


def measure(url, count, log_path):
    result = subprocess.run(
        [
            sys.executable,
            str(MEASURE),
            "--url", url,
            "--requests", str(count),
        ],
        capture_output=True,
        text=True,
        timeout=count * 3 + 10,
    )

    log_path.write_text(
        result.stdout + "\n" + result.stderr,
        encoding="utf-8",
    )

    if result.returncode != 0:
        raise RuntimeError(f"Request probe failed. Inspect {log_path}")

    rows = [
        dict(token.split("=", 1) for token in line.split())
        for line in result.stdout.splitlines()
        if line.strip()
    ]

    if len(rows) != count or any(row["status"] != "200" for row in rows):
        raise RuntimeError(f"Unexpected request results. Inspect {log_path}")

    for row in rows:
        ttfb, total = float(row["ttfb_s"]), float(row["total_s"])
        if not (math.isfinite(ttfb) and math.isfinite(total) and 0 <= ttfb <= total):
            raise RuntimeError(f"Invalid request timings. Inspect {log_path}")

    return rows


def run_case(server, round_number, args, raw):
    name = server["name"]
    port = server["port"]
    url = f"http://127.0.0.1:{port}{args.path}"
    prefix = f"round-{round_number}-{name}"
    server_log = raw / f"{prefix}-server.log"

    ensure_port_free(port)
    print(f"round={round_number} server={name} starting", flush=True)

    environment = os.environ.copy()
    environment["PORT"] = str(port)
    environment["NODE_ENV"] = "production"
    environment["RUBICON_WINDOWS_TIMER_RESOLUTION"] = args.rubicon_timer
    power_before = power_status()
    suspend_before = suspension_clock_ms()

    with server_log.open("w", encoding="utf-8") as log:
        process = subprocess.Popen(
            server["command"],
            cwd=server["cwd"],
            env=environment,
            stdout=log,
            stderr=subprocess.STDOUT,
            creationflags=subprocess.CREATE_NO_WINDOW,
        )

        try:
            wait_until_ready(process, port, server_log)
            timer_policy = process_timer_policy(process)
            (raw / f"{prefix}-environment.json").write_text(
                json.dumps({"power": power_before, "timer_policy": timer_policy}, indent=2) + "\n",
                encoding="utf-8",
            )
            if name == "rubicon" and args.rubicon_timer == "1ms" and not timer_policy["timer_resolution_explicitly_honored"]:
                raise RuntimeError("Rubicon did not explicitly honor timer resolution. Rebuild the release binary.")
            validate_page(url, args.path)

            if args.warmup:
                measure(url, args.warmup, raw / f"{prefix}-warmup.log")

            rows = measure(
                url,
                args.requests,
                raw / f"{prefix}-measured.log",
            )

            if process.poll() is not None:
                raise RuntimeError(
                    f"Server exited during measurement. Inspect {server_log}"
                )

            validate_page(url, args.path)
            power_after = power_status()
            if any(power_before[key] != power_after[key] for key in ("ac_connected", "battery_saver")):
                raise RuntimeError("Power source or battery-saver mode changed during the case; results retained but invalid.")
            if suspension_clock_ms() - suspend_before > 1000:
                raise RuntimeError("Windows suspended during the case; results retained but invalid.")
            return rows
        finally:
            if process.poll() is None:
                process.terminate()

                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)


def summarize(rows):
    ttfb = [float(row["ttfb_s"]) * 1000 for row in rows]
    total = [float(row["total_s"]) * 1000 for row in rows]

    return {
        "samples": len(rows),
        "ttfb_median_ms": statistics.median(ttfb),
        "total_mean_ms": statistics.mean(total),
        "total_median_ms": statistics.median(total),
        "total_p95_ms": sorted(total)[math.ceil(0.95 * len(total)) - 1],
        "total_p99_ms": sorted(total)[math.ceil(0.99 * len(total)) - 1],
    }


def print_summary(label, rows):
    summary = summarize(rows)
    print(
        label + " " + " ".join(
            f"{key}={value:.3f}" if isinstance(value, float) else f"{key}={value}"
            for key, value in summary.items()
        ),
        flush=True,
    )
    return summary


def write_manifest(output, args, servers, artifacts):
    def command_output(command):
        return subprocess.check_output(
            command, cwd=ROOT, text=True, encoding="utf-8",
        ).strip()

    def digest(path):
        return hashlib.sha256(path.read_bytes()).hexdigest()

    source_files = [
        path for path in (DEMO / "src").rglob("*") if path.is_file()
    ] + [
        path
        for folder in (ROOT / "crates/rubicon/src", ROOT / "crates/rubicon/js")
        for path in folder.rglob("*") if path.is_file()
    ] + [
        DEMO / "angular.json", DEMO / "package-lock.json",
        ROOT / "crates/rubicon/Cargo.toml", ROOT / "crates/rubicon/Cargo.lock",
        Path(__file__), MEASURE,
    ]
    manifest = {
        "started_at": datetime.now().astimezone().isoformat(),
        "git_head": command_output(["git", "rev-parse", "HEAD"]),
        "git_status": command_output(["git", "status", "--short"]),
        "platform": platform.platform(),
        "processor": platform.processor(),
        "logical_cpus": os.cpu_count(),
        "power": power_status(),
        "power_scheme": command_output(["powercfg.exe", "/getactivescheme"]),
        "python": sys.version,
        "node": command_output([servers[1]["command"][0], "--version"]),
        "curl": command_output(["curl.exe", "--version"]).splitlines()[0],
        "parameters": vars(args),
        "servers": [{**server, "cwd": str(server["cwd"])} for server in servers],
        "build_configuration": {
            "angular": "production / production,rubicon",
            "rust": "release, default features (render-timing disabled)",
        },
        "measurement": {
            "model": "sequential requests; one fresh curl process and TCP connection per request",
            "timings": "curl client timings, including connection; process launch excluded",
            "ttfb": "first response byte, possibly headers before HTML on Node",
            "percentiles": "pooled samples, nearest-rank; median uses statistics.median",
            "correctness": "HTML and TransferState/API gates before and after each case; HTTP status for every measured request",
            "cold_start": "not measured; correctness request precedes warmup",
            "sleep_prevention": "thread-scoped system-required request for the run; display and power plan unchanged",
            "order": "Rubicon/Node in odd rounds, Node/Rubicon in even rounds",
        },
        "sha256": {
            str(path.relative_to(ROOT)): digest(path)
            for path in source_files + artifacts
        },
    }
    (output / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8",
    )
    (output / "changes.patch").write_text(
        command_output(["git", "diff", "--no-ext-diff", "HEAD"]) + "\n",
        encoding="utf-8",
    )
    # Include new source files that are not represented by git diff HEAD yet.
    untracked = command_output([
        "git", "ls-files", "--others", "--exclude-standard", "--",
        "examples/angular-demo/src", "benchmarks/*.py",
    ]).splitlines()
    (output / "untracked-sources.json").write_text(
        json.dumps({
            path: (ROOT / path).read_text(encoding="utf-8") for path in untracked
        }, ensure_ascii=False, indent=2) + "\n", encoding="utf-8",
    )
    # Browser assets must be identical; only the server platform may differ.
    browser_hashes = []
    for variant in ("angular-demo", "rubicon"):
        folder = DEMO / "dist" / variant / "browser"
        browser_hashes.append({
            str(path.relative_to(folder)): digest(path)
            for path in folder.rglob("*") if path.is_file()
        })
    if not browser_hashes[0] or browser_hashes[0] != browser_hashes[1]:
        raise RuntimeError("Node and Rubicon browser assets differ. Rebuild both in production.")
    (output / "browser-assets.json").write_text(
        json.dumps(browser_hashes[0], indent=2) + "\n", encoding="utf-8",
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--rounds", type=int, default=4)
    parser.add_argument("--warmup", type=int, default=10)
    parser.add_argument("--requests", type=int, default=50)
    parser.add_argument("--path", choices=["/", "/products"], default="/")
    parser.add_argument("--rubicon-timer", choices=["1ms", "default"], default="1ms")
    args = parser.parse_args()

    if not 2 <= args.rounds <= 20 or args.rounds % 2:
        parser.error("--rounds must be an even number between 2 and 20")

    if not 0 <= args.warmup <= 100:
        parser.error("--warmup must be between 0 and 100")

    if not 1 <= args.requests <= 100:
        parser.error("--requests must be between 1 and 100")

    node = shutil.which("node.exe")
    if node is None:
        raise RuntimeError("node.exe was not found in PATH")

    rubicon = ROOT / "crates/rubicon/target/release/rubicon.exe"
    node_entry = DEMO / "dist/angular-demo/server/server.mjs"
    neutral_entry = DEMO / "dist/rubicon/server/server.mjs"

    for path in (MEASURE, rubicon, node_entry, neutral_entry):
        if not path.is_file():
            raise RuntimeError(f"Missing file: {path}")

    servers = [
        {
            "name": "rubicon",
            "port": 3000,
            "command": [str(rubicon)],
            "cwd": ROOT,
        },
        {
            "name": "node",
            "port": 4000,
            "command": [node, str(node_entry)],
            "cwd": DEMO,
        },
    ]

    for server in servers:
        ensure_port_free(server["port"])

    stamp = datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    output = ROOT / "benchmarks/results" / stamp
    raw = output / "raw"
    raw.mkdir(parents=True)

    print(f"Results: {output}", flush=True)
    write_manifest(output, args, servers, [rubicon, node_entry, neutral_entry])

    collected = {"rubicon": [], "node": []}
    round_summaries = []

    with (output / "samples.csv").open(
        "w", newline="", encoding="utf-8"
    ) as file:
        writer = csv.DictWriter(
            file,
            fieldnames=[
                "round", "server", "path", "request",
                "status", "ttfb_s", "total_s",
            ],
        )
        writer.writeheader()

        for round_number in range(1, args.rounds + 1):
            order = (
                servers
                if round_number % 2
                else list(reversed(servers))
            )

            for server in order:
                rows = run_case(
                    server,
                    round_number,
                    args,
                    raw,
                )

                name = server["name"]

                for row in rows:
                    writer.writerow({
                        "round": round_number,
                        "server": name,
                        "path": args.path,
                        **row,
                    })

                file.flush()
                collected[name].extend(rows)
                summary = print_summary(
                    f"round={round_number} server={name}",
                    rows,
                )
                round_summaries.append({"round": round_number, "server": name, **summary})

    print("Comparison completed.")

    overall = {}
    for name, rows in collected.items():
        print_summary(f"overall server={name}", rows)
        overall[name] = summarize(rows)

    (output / "summary.json").write_text(
        json.dumps({"path": args.path, "rounds": round_summaries, "overall": overall}, indent=2) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    try:
        with keep_system_awake():
            main()
    except KeyboardInterrupt:
        raise SystemExit("Comparison interrupted.")
    except (
        OSError,
        RuntimeError,
        ValueError,
        KeyError,
        subprocess.TimeoutExpired,
    ) as error:
        raise SystemExit(f"Comparison failed: {error}")
