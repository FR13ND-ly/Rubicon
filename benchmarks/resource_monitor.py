"""Sample process CPU time and resident memory without changing server settings."""

import ctypes
import json
import os
from pathlib import Path
import shutil
import time

import psutil


CONSOLE_HOSTS = {"conhost.exe", "openconsole.exe"}


def find_server(launcher_pid, executable, config_dir):
    """Identify the configured executable among this CLI's direct children."""
    executable = Path(executable)
    if str(executable) == executable.name:
        resolved = shutil.which(str(executable))
        if resolved is None:
            raise RuntimeError(f"Cannot resolve server executable: {executable}")
        expected = Path(resolved)
    else:
        expected = (config_dir / executable).resolve()
        if os.name == "nt" and not expected.suffix:
            expected = expected.with_suffix(".exe")

    launcher = psutil.Process(launcher_pid)
    matches = []
    for child in launcher.children():
        try:
            if os.path.normcase(child.exe()) == os.path.normcase(str(expected)):
                matches.append(child)
        except psutil.NoSuchProcess:
            continue

    if len(matches) != 1:
        raise RuntimeError(
            f"Expected one server process for {expected}, found {len(matches)}"
        )
    return matches[0]


def completed_cpu_seconds(process):
    """On Windows, retain k6's final CPU counters using its owned Popen handle."""
    if os.name != "nt":
        return None

    from ctypes import wintypes

    get_times = ctypes.WinDLL("kernel32", use_last_error=True).GetProcessTimes
    get_times.argtypes = [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4
    get_times.restype = wintypes.BOOL
    created, exited, kernel, user = (wintypes.FILETIME() for _ in range(4))
    if not get_times(
        int(process._handle),
        ctypes.byref(created),
        ctypes.byref(exited),
        ctypes.byref(kernel),
        ctypes.byref(user),
    ):
        raise ctypes.WinError(ctypes.get_last_error())
    ticks = sum(
        (value.dwHighDateTime << 32) | value.dwLowDateTime
        for value in (kernel, user)
    )
    return ticks / 10_000_000


class ProcessGroup:
    def __init__(self, root, descendants=False, lifetime=False):
        self.root = root
        self.descendants = descendants
        self.lifetime = lifetime
        self.processes = {}

    def capture(self, initial=False, discover=True):
        # Keep counters fresh while reusing membership between process scans.
        discover = discover or not self.processes
        candidates = [self.root] if discover else []
        if discover and self.descendants and self.root.is_running():
            try:
                candidates.extend(self.root.children(recursive=True))
            except psutil.NoSuchProcess:
                pass

        for process in candidates:
            try:
                identity = (process.pid, process.create_time())
                if identity not in self.processes:
                    name = process.name()
                    if name.casefold() in CONSOLE_HOSTS:
                        continue
                    self.processes[identity] = {
                        "process": process,
                        "pid": process.pid,
                        "name": name,
                        "created_at": identity[1],
                        "baseline_cpu_s": None,
                        "last_cpu_s": 0.0,
                        "lost": False,
                        "finalized": False,
                    }
            except psutil.NoSuchProcess:
                continue

        rss = 0
        private = 0 if os.name == "nt" else None
        live = 0
        for state in self.processes.values():
            process = state["process"]
            try:
                if not process.is_running():
                    raise psutil.NoSuchProcess(process.pid)
                with process.oneshot():
                    cpu = process.cpu_times()
                    memory = process.memory_info()
                total = cpu.user + cpu.system
                if state["baseline_cpu_s"] is None:
                    state["baseline_cpu_s"] = (
                        total if initial and not self.lifetime else 0.0
                    )
                state["last_cpu_s"] = max(state["last_cpu_s"], total)
                rss += memory.rss
                if private is not None:
                    private += memory.private
                live += 1
            except psutil.NoSuchProcess:
                state["lost"] = True

        return {
            "cpu_seconds": sum(
                max(0.0, state["last_cpu_s"] - (state["baseline_cpu_s"] or 0.0))
                for state in self.processes.values()
            ),
            "rss_bytes": rss,
            "private_bytes": private,
            "live_processes": live,
        }

    def finalize_root(self, cpu_seconds):
        if cpu_seconds is None:
            return
        identity = (self.root.pid, self.root.create_time())
        state = self.processes[identity]
        state["last_cpu_s"] = max(state["last_cpu_s"], cpu_seconds)
        state["finalized"] = True

    def metadata(self):
        return [
            {
                key: state[key]
                for key in ("pid", "name", "created_at", "lost", "finalized")
            }
            for state in self.processes.values()
        ]


class ResourceMonitor:
    def __init__(self, server, launcher, sample_ms, process_scan_ms=1000):
        self.groups = {
            "server": ProcessGroup(server, descendants=True),
            "cli": ProcessGroup(launcher),
            "coordinator": ProcessGroup(psutil.Process()),
        }
        self.sample_ms = sample_ms
        self.process_scan_ms = process_scan_ms
        self.next_process_scan = 0.0
        self.samples = []
        self.started = time.perf_counter()
        self.sample(initial=True)

    def add_generator(self, pid):
        self.groups["k6"] = ProcessGroup(
            psutil.Process(pid), descendants=True, lifetime=True
        )

    def sample(self, initial=False, force_discovery=False):
        started = time.perf_counter()
        discover = initial or force_discovery or started >= self.next_process_scan
        if discover:
            self.next_process_scan = started + self.process_scan_ms / 1000
        readings = {
            name: group.capture(initial=initial, discover=discover)
            for name, group in self.groups.items()
        }
        self.samples.append(
            {
                "elapsed_s": time.perf_counter() - self.started,
                "collection_duration_s": time.perf_counter() - started,
                "groups": readings,
            }
        )

    def save(self, path, phase, backend, requests, exit_code, error=None):
        duration = self.samples[-1]["elapsed_s"] - self.samples[0]["elapsed_s"]
        summary = {}
        for name, group in self.groups.items():
            readings = [
                sample["groups"][name]
                for sample in self.samples
                if name in sample["groups"]
            ]
            cpu = readings[-1]["cpu_seconds"]
            private = [
                reading["private_bytes"]
                for reading in readings
                if reading["private_bytes"] is not None
            ]
            summary[name] = {
                "cpu_seconds": cpu,
                "cpu_mean_percent": cpu / duration * 100 if duration > 0 else None,
                "cpu_time_complete": all(
                    not state["lost"] or state["finalized"]
                    for state in group.processes.values()
                ),
                "rss_peak_mib": max(reading["rss_bytes"] for reading in readings)
                / (1024 * 1024),
                "private_peak_mib": max(private) / (1024 * 1024) if private else None,
                "processes": group.metadata(),
            }

        report = {
            "schema_version": 1,
            "phase": phase,
            "backend": backend,
            "requested_requests": requests,
            "k6_exit_code": exit_code,
            "error": error,
            "duration_s": duration,
            "collection_duration_s": sum(
                sample["collection_duration_s"] for sample in self.samples
            ),
            "sample_interval_ms": self.sample_ms,
            "process_scan_interval_ms": self.process_scan_ms,
            "logical_cpu_count": psutil.cpu_count(),
            "total_ram_bytes": psutil.virtual_memory().total,
            "psutil_version": psutil.__version__,
            "definitions": {
                "window": (
                    "From the baseline before k6 launch through the final sample "
                    "after its exit. Includes k6 initialization and reporting. "
                    "Excludes server startup and other phases."
                ),
                "cpu": (
                    "Delta of user + system CPU time. 100% means one logical CPU "
                    "fully busy; may exceed 100%."
                ),
                "ram": (
                    "Maximum observed sum of process RSS (Windows working set), "
                    "in MiB. Sampling may miss brief peaks; shared pages can be "
                    "counted more than once."
                ),
                "private": (
                    "Windows private committed bytes, not private resident RAM; "
                    "null on other platforms."
                ),
                "processes": (
                    "Server and k6 include observed descendants. CLI and "
                    "coordinator exclude descendants. Console hosts are excluded."
                ),
                "discovery": (
                    "Process membership is refreshed at process_scan_interval_ms "
                    "and at phase boundaries. Known process counters are read at "
                    "sample_interval_ms. A newly attached group is scanned immediately."
                ),
                "complete": (
                    "False means a tracked process exited before its final CPU "
                    "counters could be read. Processes that start and exit "
                    "between process scans may be missed."
                ),
            },
            "summary": summary,
            "samples": self.samples,
        }
        path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(f"\nResources: {path}", flush=True)
        for name, values in summary.items():
            qualifier = "" if values["cpu_time_complete"] else " (CPU lower bound)"
            print(
                f"  {name}: CPU {values['cpu_mean_percent']:.1f}% / "
                f"{values['cpu_seconds']:.3f}s; RAM RSS peak "
                f"{values['rss_peak_mib']:.1f} MiB{qualifier}",
                flush=True,
            )
        return report
