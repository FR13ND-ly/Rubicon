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


def planned_runs(vus_levels, rounds):
    plan = []
    for round_index in range(rounds):
        levels = vus_levels if round_index % 2 == 0 else list(reversed(vus_levels))
        for vus in levels:
            backends = ["rubicon", "node"]
            if (round_index + vus_levels.index(vus)) % 2:
                backends.reverse()
            for backend in backends:
                plan.append({"round": round_index + 1, "vus": vus, "backend": backend})
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
    parser.add_argument("--rounds", type=int, default=2)
    parser.add_argument("--vus", type=int, nargs="+", default=[1, 4, 16])
    parser.add_argument("--warmup", type=int, default=100)
    parser.add_argument("--requests", type=int, default=1000)
    parser.add_argument("--sample-ms", type=int, default=100)
    parser.add_argument("--process-scan-ms", type=int, default=1000)
    args = parser.parse_args(argv)
    if args.rounds < 1 or args.warmup < 0:
        parser.error("--rounds must be positive and --warmup must be nonnegative")
    if any(vus < 1 for vus in args.vus) or len(set(args.vus)) != len(args.vus):
        parser.error("--vus must contain distinct positive integers")
    if args.requests < max(args.vus):
        parser.error("--requests must be at least the largest VU count")
    if args.sample_ms < 10 or args.process_scan_ms < args.sample_ms:
        parser.error("Sampling must be at least 10 ms; process scans must not be more frequent than samples")

    config_path = args.config.resolve()
    config = tomllib.loads(config_path.read_text(encoding="utf-8"))
    frozen = snapshot(config_path, config)
    plan = planned_runs(args.vus, args.rounds)
    stamp = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
    directory = ROOT / "benchmarks/results" / f"comparison-{stamp}-{uuid.uuid4().hex[:8]}"
    directory.mkdir(parents=True)
    manifest = {
        "schema_version": 1, "started_utc": datetime.now(timezone.utc).isoformat(),
        "completed": False, "artifacts_unchanged": None,
        "config": str(config_path), "parameters": vars(args) | {"config": str(config_path)},
        "renderer": {"max_in_flight": 16, "queue_capacity": 16} | config.get("renderer", {}),
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
        },
        "method": "Fresh server per run; warmup at 1 VU; fixed shared iterations; servers sequential; backend and VU order alternated; no CPU/RAM caps.",
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
            backend, vus = item["backend"], item["vus"]
            child = directory / f"{sequence:02d}-round{item['round']}-vu{vus}-{backend}"
            print(f"\n[{sequence}/{len(plan)}] round={item['round']} backend={backend} vus={vus}", flush=True)
            error = None
            code = None
            try:
                code = run.main([
                    "--backend", backend, "--config", str(config_path),
                    "--warmup", str(args.warmup), "--requests", str(args.requests), "--vus", str(vus),
                    "--sample-ms", str(args.sample_ms), "--process-scan-ms", str(args.process_scan_ms),
                    "--output-dir", str(child), "--capture-k6-output",
                ])
            except (OSError, RuntimeError, subprocess.TimeoutExpired, run.psutil.Error) as exception:
                error = f"{type(exception).__name__}: {exception}"
                print(error, file=sys.stderr, flush=True)
            except KeyboardInterrupt:
                interrupted = measurement(child, backend, vus, args.requests, None, "Interrupted by user")
                interrupted.update(sequence=sequence, round=item["round"])
                runs.append(interrupted)
                raise
            result = measurement(child, backend, vus, args.requests, code, error)
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

    print("\nBackend  VUs  Valid  Correct/s  Mean ms  p95 ms/run  CPU ms/correct  RSS peak MiB")
    for group in report["groups"]:
        server = group["resources"].get("server", {})
        peaks = server.get("rss_peak_mib_per_run", [])
        print(
            f"{group['backend']:8} {group['vus']:3} {str(group['all_valid']):5} "
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
