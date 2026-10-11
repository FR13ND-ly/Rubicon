"""Windows-only benchmark helper; never attaches to an existing console."""

import argparse
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[2]


def save(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
    temporary.replace(path)


def inside(path):
    resolved = Path(path).resolve(strict=True)
    if not resolved.is_relative_to(ROOT):
        raise ValueError("Helper inputs must remain in the repository")
    return resolved


def worker(args, binary, directory):
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.DWORD)
    kernel.SetConsoleCtrlHandler.argtypes = [callback_type, wintypes.BOOL]
    kernel.SetConsoleCtrlHandler.restype = wintypes.BOOL
    kernel.GetConsoleProcessList.argtypes = [ctypes.POINTER(wintypes.DWORD), wintypes.DWORD]
    kernel.GetConsoleProcessList.restype = wintypes.DWORD
    kernel.GenerateConsoleCtrlEvent.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.GenerateConsoleCtrlEvent.restype = wintypes.BOOL
    kernel.SetProcessAffinityMask.argtypes = [wintypes.HANDLE, ctypes.c_size_t]
    kernel.SetProcessAffinityMask.restype = wintypes.BOOL

    # A handler is process-local; unlike the NULL-handler ignore attribute,
    # registering this callback does not make the child ignore Ctrl+C.
    handler = callback_type(lambda event: event == 0)
    if not kernel.SetConsoleCtrlHandler(handler, True):
        raise ctypes.WinError(ctypes.get_last_error())

    def members():
        values = (wintypes.DWORD * 32)()
        count = kernel.GetConsoleProcessList(values, len(values))
        if count == 0 or count > len(values):
            raise RuntimeError("Cannot verify the private console's membership")
        return set(values[:count])

    # This mode is only launched with CREATE_NEW_CONSOLE by the supervisor.
    if members() != {os.getpid()}:
        raise RuntimeError("Refusing to use a console shared with other processes")
    child = None
    result = {"helper_pid": os.getpid(), "forced": False, "status": "starting"}
    started = time.monotonic()
    try:
        with (directory / "server.stdout.log").open("wb") as stdout, (directory / "server.stderr.log").open("wb") as stderr:
            child = subprocess.Popen([str(binary)], cwd=ROOT, stdin=subprocess.DEVNULL,
                                     stdout=stdout, stderr=stderr, creationflags=0)
            if not kernel.SetProcessAffinityMask(int(child._handle), args.affinity):
                raise ctypes.WinError(ctypes.get_last_error())
            deadline = time.monotonic() + 5
            while members() != {os.getpid(), child.pid}:
                if child.poll() is not None or time.monotonic() >= deadline:
                    raise RuntimeError("Server did not join the private console")
                time.sleep(0.02)
            result.update(server_pid=child.pid, console_members=sorted(members()), status="running")
            save(directory / "server-process.json", result)
            stop = directory / "stop.request"
            while child.poll() is None and not stop.exists():
                if time.monotonic() - started > args.max_seconds:
                    raise TimeoutError("Benchmark server lifetime limit exceeded")
                time.sleep(0.05)
            if child.poll() is not None:
                raise RuntimeError("Server exited before the requested normal stop")
            # CTRL_C_EVENT cannot target a process group. Broadcast only after
            # proving this newly created console contains exactly our two PIDs.
            checked = sorted(members())
            if set(checked) != {os.getpid(), child.pid}:
                raise RuntimeError("Console membership changed; refusing to signal")
            result["console_members_at_signal"] = checked
            if not kernel.GenerateConsoleCtrlEvent(0, 0):
                raise ctypes.WinError(ctypes.get_last_error())
            result["signal"] = "CTRL_C_EVENT in verified private console"
            result["server_exit_code"] = child.wait(timeout=15)
            result["status"] = "completed" if child.returncode == 0 else "failed"
    except Exception as error:
        result.update(status="failed", error=str(error))
    finally:
        if child is not None and child.poll() is None:
            result["forced"] = True
            child.kill()  # Only the exact subprocess created above.
            result["server_exit_code"] = child.wait(timeout=5)
        result["elapsed_seconds"] = time.monotonic() - started
        save(directory / "shutdown.json", result)
    return 0 if result["status"] == "completed" and not result["forced"] else 1


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True)
    parser.add_argument("--directory", required=True)
    parser.add_argument("--affinity", type=int, default=1)
    parser.add_argument("--max-seconds", type=int, default=120)
    parser.add_argument("--worker", action="store_true")
    args = parser.parse_args()
    if sys.platform != "win32":
        raise RuntimeError("The diagnostic console helper requires Windows")
    binary, directory = inside(args.binary), inside(args.directory)
    if args.worker:
        return worker(args, binary, directory)
    startup = subprocess.STARTUPINFO()
    startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = subprocess.SW_HIDE
    command = [sys.executable, str(Path(__file__).resolve()), "--worker",
               "--binary", str(binary), "--directory", str(directory),
               "--affinity", str(args.affinity), "--max-seconds", str(args.max_seconds)]
    with subprocess.Popen(command, cwd=ROOT, creationflags=subprocess.CREATE_NEW_CONSOLE,
                          startupinfo=startup, stdin=subprocess.DEVNULL,
                          stdout=sys.stdout, stderr=sys.stderr) as console:
        return console.wait(timeout=args.max_seconds + 25)


if __name__ == "__main__":
    sys.exit(main())
