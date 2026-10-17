import json
import os
from pathlib import Path
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import run
from compare import planned_runs


class WarmupTests(unittest.TestCase):
    def setUp(self):
        root = Path(__file__).resolve().parents[2]
        self.temporary = tempfile.TemporaryDirectory(dir=root / "target")
        self.directory = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)

    def coverage(self, text, workers=2, minimum=20):
        path = self.directory / "server.stderr.log"
        path.write_text(text, encoding="utf-8")
        return run.warmup_coverage(path, workers, minimum)

    def test_unordered_confirmations_cover_every_runtime(self):
        result = self.coverage(
            "RUBICON_DEMO_WARMUP_READY runtime_id=2 completed=20\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=20\n"
        )
        self.assertTrue(result["verified"])
        self.assertEqual(result["confirmations"], {1: 20, 2: 20})

    def test_missing_duplicate_unexpected_and_wrong_threshold_fail(self):
        invalid_logs = [
            "RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=20\n",
            "RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=20\n" * 2
            + "RUBICON_DEMO_WARMUP_READY runtime_id=2 completed=20\n",
            "RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=20\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=3 completed=20\n",
            "RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=19\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=2 completed=20\n",
            "RUBICON_DEMO_WARMUP_READY malformed\n",
        ]
        for text in invalid_logs:
            with self.subTest(text=text):
                result = self.coverage(text)
                self.assertFalse(result["verified"])
                self.assertTrue(result["problems"])

    def test_variants_rotate_through_every_position_at_a_fixed_active_budget(self):
        plan = planned_runs([1, 4, 16], 4, [1, 2, 4], 16, 7)
        self.assertEqual(len(plan), 48)
        for vus in [1, 4, 16]:
            positions = {}
            for round_ in range(1, 5):
                items = [item for item in plan if item["round"] == round_ and item["vus"] == vus]
                self.assertEqual(len(items), 4)
                for position, item in enumerate(items):
                    positions.setdefault((item["backend"], item["workers"]), []).append(position)
                    if item["backend"] == "rubicon":
                        self.assertEqual(item["workers"] * item["renderer"]["max_in_flight"], 16)
                        self.assertEqual(item["renderer"]["queue_capacity"], 7)
            for values in positions.values():
                self.assertEqual(sorted(values), [0, 1, 2, 3])

    def configuration(self):
        return {
            "server": {"bind": "127.0.0.1:3000", "origin": "http://127.0.0.1:3000"},
            "renderer": {"workers": 1, "max_in_flight": 16, "queue_capacity": 16},
            "rubicon": {"executable": "../../target/release/server", "server_entry": "dist/rubicon/server.mjs", "browser_dir": "dist/rubicon/browser"},
            "node": {"executable": "node", "server_entry": "dist/node/server.mjs", "browser_dir": "dist/node/browser"},
        }

    def source_config(self):
        source = self.directory / "source.toml"
        config = self.configuration()
        run.write_effective_config(source, source, config, config["renderer"])
        return source

    def test_variant_config_resolves_paths_and_preserves_original(self):
        source = self.source_config()
        before = source.read_bytes()
        config = self.configuration()
        destination = self.directory / "nested" / "effective.toml"
        destination.parent.mkdir()
        renderer = {"workers": 4, "max_in_flight": 4, "queue_capacity": 7}
        run.write_effective_config(destination, source, config, renderer)
        effective = tomllib.loads(destination.read_text(encoding="utf-8"))
        self.assertEqual(effective["renderer"], renderer)
        self.assertEqual(effective["node"]["executable"], "node")
        self.assertEqual(Path(effective["rubicon"]["server_entry"]), (source.parent / config["rubicon"]["server_entry"]).resolve())
        self.assertEqual(source.read_bytes(), before)

    def exercise_runner(self, confirmations, semi=0, transport=None, threads=None):
        source = self.source_config()
        destination = self.directory / "result"
        process = Mock()
        process.wait.return_value = 0
        def spawn(*args, **kwargs):
            kwargs["stderr"].write(confirmations.encode("utf-8"))
            kwargs["stderr"].flush()
            return process
        with (
            patch.object(run.subprocess, "Popen", side_effect=spawn) as popen,
            patch.object(run, "wait_for_server"),
            patch.object(run, "find_server"),
            patch.object(run.psutil, "Process"),
            patch.object(run, "run_k6", return_value=0) as k6,
            patch.dict(os.environ, {"RUBICON_DEMO_WARMUP_REQUESTS": "999", "RUBICON_DEMO_V8_SEMI_SPACE_MIB": "64", "RUBICON_DEMO_BODY_TRANSPORT": "inherited", "TOKIO_WORKER_THREADS": "99"}),
        ):
            arguments = ["--config", str(source), "--output-dir", str(destination),
                         "--workers", "2", "--max-in-flight", "8", "--queue-capacity", "7",
                         "--warmup", "4", "--warmup-vus", "2", "--warmup-per-worker", "2", "--requests", "4",
                         "--semi-space-mib", str(semi)]
            if transport:
                arguments.extend(["--body-transport", transport])
            if threads is not None:
                arguments.extend(["--http-threads", str(threads)])
            error = None
            try:
                code = run.main(arguments)
            except RuntimeError as exception:
                code, error = None, str(exception)
            self.assertEqual(os.environ["RUBICON_DEMO_WARMUP_REQUESTS"], "999")
            self.assertEqual(os.environ["RUBICON_DEMO_V8_SEMI_SPACE_MIB"], "64")
            self.assertEqual(os.environ["RUBICON_DEMO_BODY_TRANSPORT"], "inherited")
            self.assertEqual(os.environ["TOKIO_WORKER_THREADS"], "99")
            self.assertEqual(popen.call_args.kwargs["env"]["RUBICON_DEMO_WARMUP_REQUESTS"], "2")
            self.assertEqual(popen.call_args.kwargs["env"].get("RUBICON_DEMO_V8_SEMI_SPACE_MIB"), str(semi) if semi else None)
            self.assertEqual(popen.call_args.kwargs["env"].get("RUBICON_DEMO_BODY_TRANSPORT"), transport)
            self.assertEqual(popen.call_args.kwargs["env"].get("TOKIO_WORKER_THREADS"), str(threads) if threads else None)
            self.assertEqual(process.stdin.close.call_count, 1)
            return code, error, [call.args[3] for call in k6.call_args_list]

    def test_measurement_is_not_started_when_a_runtime_is_unwarmed(self):
        code, error, phases = self.exercise_runner(
            "RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=2\n"
        )
        self.assertIsNone(code)
        self.assertIn("Missing warmup confirmations", error)
        self.assertEqual(phases, ["Warmup"])

    def test_verified_warmup_allows_measurement_and_scopes_the_child_environment(self):
        code, error, phases = self.exercise_runner(
            "RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=2\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=2 completed=2\n"
        )
        self.assertEqual(code, 0)
        self.assertIsNone(error)
        self.assertEqual(phases, ["Warmup", "Measurement"])

    def test_memory_profile_requires_server_confirmation_before_load(self):
        code, error, phases = self.exercise_runner(
            "RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=2\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=2 completed=2\n",
            semi=8,
        )
        self.assertIsNone(code)
        self.assertIn("requested V8 memory profile", error)
        self.assertEqual(phases, [])

    def test_confirmed_memory_profile_allows_the_normal_warmup_and_measurement(self):
        code, error, phases = self.exercise_runner(
            "RUBICON_DEMO_V8_SEMI_SPACE_MIB=8\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=2\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=2 completed=2\n",
            semi=8,
        )
        self.assertEqual(code, 0)
        self.assertIsNone(error)
        self.assertEqual(phases, ["Warmup", "Measurement"])

    def test_memory_profiles_rotate_independently(self):
        plan = planned_runs([16], 4, [4], 16, 16, [0, 8, 16])
        self.assertEqual(len(plan), 16)
        positions = {}
        for round_ in range(1, 5):
            items = [item for item in plan if item["round"] == round_]
            for position, item in enumerate(items):
                positions.setdefault((item["backend"], item["semi_space_mib"]), []).append(position)
        self.assertEqual(len(positions), 4)
        self.assertTrue(all(sorted(values) == [0, 1, 2, 3] for values in positions.values()))

    def test_unconfirmed_body_transport_blocks_all_load(self):
        code, error, phases = self.exercise_runner("", transport="bytes")
        self.assertIsNone(code)
        self.assertIn("requested body transport", error)
        self.assertEqual(phases, [])

    def test_confirmed_byte_transport_is_scoped_to_the_child(self):
        code, error, phases = self.exercise_runner(
            "RUBICON_DEMO_BODY_TRANSPORT=bytes\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=2\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=2 completed=2\n", transport="bytes",
        )
        self.assertEqual(code, 0)
        self.assertIsNone(error)
        self.assertEqual(phases, ["Warmup", "Measurement"])

    def test_pool_and_transport_matrix_rotates_every_variant(self):
        plan = planned_runs([16], 13, [2, 4], [4, 8, 16], 16, [16], ["text", "bytes"])
        positions = {}
        for round_ in range(1, 14):
            items = [item for item in plan if item["round"] == round_]
            self.assertEqual(len(items), 13)
            for position, item in enumerate(items):
                renderer = item["renderer"]
                key = (item["backend"], item["workers"],
                       renderer["max_in_flight"] if renderer else None, item["body_transport"])
                positions.setdefault(key, []).append(position)
                if renderer:
                    self.assertIn(renderer["max_in_flight"] * item["workers"], [4, 8, 16])
        self.assertEqual(len(positions), 13)
        self.assertTrue(all(sorted(values) == list(range(13)) for values in positions.values()))

    def test_http_thread_confirmations_reject_missing_duplicate_invalid_and_wrong_counts(self):
        path = self.directory / "http.log"
        for text in ["", "RUBICON_DEMO_HTTP_THREADS=0\n", "RUBICON_DEMO_HTTP_THREADS=two\n",
                     "RUBICON_DEMO_HTTP_THREADS=2\n" * 2, "RUBICON_DEMO_HTTP_THREADS=4\n"]:
            path.write_text(text)
            with self.subTest(text=text), self.assertRaises(RuntimeError):
                run.confirmed_http_threads(path, 2)

    def test_http_default_clears_inherited_override_and_records_effective_count(self):
        code, error, phases = self.exercise_runner(
            "RUBICON_DEMO_HTTP_THREADS=16\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=2\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=2 completed=2\n", threads=0,
        )
        self.assertEqual(code, 0)
        self.assertIsNone(error)
        self.assertEqual(phases, ["Warmup", "Measurement"])
        metadata = json.loads((self.directory / "result/run.json").read_text())
        self.assertEqual((metadata["http_threads"], metadata["http_threads_actual"]), (0, 16))

    def test_http_override_is_scoped_to_the_child_and_confirmed_before_load(self):
        code, error, phases = self.exercise_runner(
            "RUBICON_DEMO_HTTP_THREADS=2\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=1 completed=2\n"
            "RUBICON_DEMO_WARMUP_READY runtime_id=2 completed=2\n", threads=2,
        )
        self.assertEqual(code, 0)
        self.assertIsNone(error)
        self.assertEqual(phases, ["Warmup", "Measurement"])

    def test_http_mismatch_prevents_all_load(self):
        code, error, phases = self.exercise_runner("RUBICON_DEMO_HTTP_THREADS=4\n", threads=2)
        self.assertIsNone(code)
        self.assertIn("Requested 2 HTTP threads", error)
        self.assertEqual(phases, [])

    def test_http_profiles_rotate_independently_without_adding_node_runs(self):
        plan = planned_runs([4, 16], 4, [4], [16], 16, [16], ["text"], [0, 1, 2, 4], ["rubicon"])
        self.assertEqual(len(plan), 32)
        for vus in [4, 16]:
            positions = {threads: [] for threads in [0, 1, 2, 4]}
            for round_ in range(1, 5):
                items = [item for item in plan if item["round"] == round_ and item["vus"] == vus]
                for position, item in enumerate(items):
                    positions[item["http_threads"]].append(position)
                    self.assertEqual(item["backend"], "rubicon")
                    self.assertEqual(item["renderer"], {"workers": 4, "max_in_flight": 4, "queue_capacity": 16})
            self.assertTrue(all(sorted(values) == [0, 1, 2, 3] for values in positions.values()))


if __name__ == "__main__":
    unittest.main()
