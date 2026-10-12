from contextlib import contextmanager
import os
from pathlib import Path
import subprocess
import sys
import unittest

import psutil

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from resource_monitor import ProcessGroup, completed_cpu_seconds


@contextmanager
def worker(source):
    process = subprocess.Popen(
        [sys._base_executable, "-u", "-c", source],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        yield process
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
        process.stdout.close()
        process.stderr.close()


class ResourceCollectionTests(unittest.TestCase):
    def test_phase_excludes_prior_cpu_and_includes_new_child_cpu_and_ram(self):
        leaf = """
import sys, time
memory = bytearray(32 * 1024 * 1024)
deadline = time.process_time() + 0.3
while time.process_time() < deadline:
    sum(range(1000))
print('CHILD_READY', flush=True)
sys.stdin.read()
"""
        parent = f"""
import subprocess, sys, time
deadline = time.process_time() + 0.3
while time.process_time() < deadline:
    sum(range(1000))
print('READY', flush=True)
sys.stdin.readline()
child = subprocess.Popen(
    [sys.executable, '-u', '-c', {leaf!r}],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
)
print(child.stdout.readline().strip(), flush=True)
sys.stdin.readline()
child.stdin.close()
child.wait(timeout=5)
print('CHILD_EXITED', flush=True)
sys.stdin.read()
"""
        with worker(parent) as process:
            self.assertEqual(process.stdout.readline().strip(), "READY")
            group = ProcessGroup(psutil.Process(process.pid), descendants=True)
            before = group.capture(initial=True)
            self.assertEqual(before["cpu_seconds"], 0)

            process.stdin.write("spawn\n")
            process.stdin.flush()
            self.assertEqual(process.stdout.readline().strip(), "CHILD_READY")
            during = group.capture()
            self.assertEqual(during["live_processes"], 2)
            self.assertGreater(during["cpu_seconds"], 0.25)
            self.assertGreater(during["rss_bytes"] - before["rss_bytes"], 24 * 1024 * 1024)

            process.stdin.write("stop child\n")
            process.stdin.flush()
            self.assertEqual(process.stdout.readline().strip(), "CHILD_EXITED")
            after = group.capture()
            self.assertEqual(after["live_processes"], 1)
            self.assertGreaterEqual(after["cpu_seconds"], during["cpu_seconds"])
            self.assertTrue(any(state["lost"] for state in group.metadata()))

    @unittest.skipUnless(os.name == "nt", "Windows final CPU accounting")
    def test_final_cpu_is_read_after_generator_exit(self):
        source = """
import time
deadline = time.process_time() + 0.3
while time.process_time() < deadline:
    sum(range(1000))
"""
        with worker(source) as process:
            group = ProcessGroup(psutil.Process(process.pid), lifetime=True)
            group.capture()
            self.assertEqual(process.wait(timeout=5), 0)
            cpu = completed_cpu_seconds(process)
            self.assertGreater(cpu, 0.25)
            group.finalize_root(cpu)
            self.assertAlmostEqual(group.capture()["cpu_seconds"], cpu)
            self.assertTrue(group.metadata()[0]["finalized"])


if __name__ == "__main__":
    unittest.main()
