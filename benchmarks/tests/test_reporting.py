import copy
import hashlib
import json
from contextlib import contextmanager
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare import planned_runs
from reporting import OUTCOMES, group_results, measurement, write_report


@contextmanager
def temporary_directory():
    workspace = Path(__file__).resolve().parents[2]
    with tempfile.TemporaryDirectory(dir=workspace / "target") as path:
        directory = Path(path).resolve()
        directory.relative_to(workspace.resolve())
        yield directory


class ComparisonTests(unittest.TestCase):
    def record(self, directory, count=100, rate=50, mean=10, cpu=0.5, correct=None):
        correct = count if correct is None else correct
        metrics = {"http_reqs": {"count": count, "rate": rate}}
        for name, key in OUTCOMES.items():
            metrics[key] = {"count": correct if name == "correct" else count - correct if name == "unavailable" else 0}
        metrics["correct_response_duration"] = {"avg": mean, "p(95)": mean + 1, "p(99)": mean + 2}
        metrics["correct_response_ttfb"] = {"avg": mean - 1}
        directory.joinpath("measurement.k6.json").write_text(json.dumps({"metrics": metrics}))
        resources = {
            "phase": "Measurement", "backend": "rubicon", "vus": 4,
            "requested_requests": count, "k6_exit_code": 0, "error": None,
            "summary": {
                name: {"cpu_seconds": cpu, "cpu_mean_percent": 25, "cpu_time_complete": True,
                       "rss_peak_mib": 100, "private_peak_mib": None}
                for name in ["server", "cli", "k6", "coordinator"]
            },
        }
        directory.joinpath("measurement.resources.json").write_text(json.dumps(resources))
        return measurement(directory, "rubicon", 4, count, 0)

    def test_unequal_run_lengths_use_correct_weights_and_preserve_percentiles(self):
        with temporary_directory() as path:
            directory = Path(path)
            first = self.record(directory, count=100, rate=50, mean=10, cpu=0.5)
            second = self.record(directory, count=900, rate=300, mean=30, cpu=2)
            group = group_results([first, second])[0]
        self.assertAlmostEqual(group["correct_rps"], 200)
        self.assertAlmostEqual(group["mean_ms"], 28)
        self.assertEqual(group["p95_ms_per_run"], [11, 31])
        self.assertEqual(group["p99_ms_per_run"], [12, 32])
        self.assertAlmostEqual(group["resources"]["server"]["cpu_ms_per_correct_request"], 2.5)

    def test_fast_errors_are_retained_and_invalidate_the_comparison(self):
        with temporary_directory() as path:
            directory = Path(path)
            record = self.record(directory, correct=10, rate=1000)
            report = write_report(directory, [record], {
                "completed": True, "artifacts_unchanged": True, "run_order": [1],
            })
            self.assertFalse(report["all_valid"])
            self.assertEqual(record["counts"]["unavailable"], 90)
            self.assertAlmostEqual(report["groups"][0]["correct_rps"], 100)

    def test_missing_samples_and_bad_accounting_do_not_disappear(self):
        with temporary_directory() as path:
            directory = Path(path)
            missing = measurement(directory, "rubicon", 4, 100, 1)
            self.assertFalse(missing["valid"])
            self.record(directory)
            metrics_path = directory / "measurement.k6.json"
            raw = json.loads(metrics_path.read_text())
            raw["metrics"]["correct_responses"]["count"] = 99
            metrics_path.write_text(json.dumps(raw))
            inconsistent = measurement(directory, "rubicon", 4, 100, 0)
            groups = group_results([missing, inconsistent])
            self.assertEqual(groups[0]["runs"], 2)
            self.assertFalse(groups[0]["all_valid"])
            self.assertIn("Request accounting is incomplete or inconsistent", inconsistent["problems"])

    def test_incomplete_cpu_is_not_reported_as_an_exact_cost(self):
        with temporary_directory() as path:
            directory = Path(path)
            self.record(directory)
            resources_path = directory / "measurement.resources.json"
            raw = json.loads(resources_path.read_text())
            raw["summary"]["server"]["cpu_time_complete"] = False
            resources_path.write_text(json.dumps(raw))
            result = measurement(directory, "rubicon", 4, 100, 0)
            group = group_results([result])[0]
            self.assertIsNone(group["resources"]["server"]["cpu_ms_per_correct_request"])

    def test_every_level_gets_both_backend_orders_across_two_rounds(self):
        plan = planned_runs([1, 4, 16], 2)
        for vus in [1, 4, 16]:
            orders = [[item["backend"] for item in plan if item["round"] == round_ and item["vus"] == vus] for round_ in [1, 2]]
            self.assertEqual(orders[0], list(reversed(orders[1])))
        self.assertEqual(len(plan), 12)

    def test_worker_counts_and_active_limits_form_separate_groups(self):
        with temporary_directory() as directory:
            first = self.record(directory)
            first.update(workers=1, renderer={"workers": 1, "max_in_flight": 16, "queue_capacity": 16})
            second = copy.deepcopy(first)
            second.update(workers=4, renderer={"workers": 4, "max_in_flight": 4, "queue_capacity": 16})
            third = copy.deepcopy(first)
            third["renderer"]["max_in_flight"] = 8
            fourth = copy.deepcopy(second)
            fourth["semi_space_mib"] = 8
            groups = group_results([first, second, third, fourth])
            self.assertEqual(len(groups), 4)
            self.assertEqual(sorted(group["total_in_flight"] for group in groups), [8, 16, 16, 16])
            self.assertTrue(all(group["requests"] == 100 for group in groups))

    def execution_record(self, directory, confirmations):
        self.record(directory)
        renderer = {"workers": 2, "max_in_flight": 8, "queue_capacity": 16}
        configuration = directory / "effective.toml"
        configuration.write_text("[renderer]\nworkers=2\nmax_in_flight=8\nqueue_capacity=16\n")
        metadata = {
            "backend": "rubicon", "workers": 2, "renderer": renderer,
            "effective_config": str(configuration),
            "effective_config_sha256": hashlib.sha256(configuration.read_bytes()).hexdigest(),
            "requests": 100, "vus": 4, "warmup_requests": 4,
            "warmup_vus": 2, "warmup_per_worker": 2,
        }
        directory.joinpath("run.json").write_text(json.dumps(metadata))
        directory.joinpath("warmup.json").write_text(json.dumps({
            "backend": "rubicon", "workers": 2, "requests": 4, "vus": 2,
            "minimum_per_worker": 2, "performed": True, "verified": True,
            "required": True, "confirmations": confirmations, "problems": [],
        }))
        directory.joinpath("warmup.k6.json").write_text(json.dumps({
            "metrics": {"http_reqs": {"count": 4}, "correct_responses": {"count": 4}},
        }))
        return renderer

    def test_missing_runtime_confirmation_invalidates_otherwise_correct_results(self):
        with temporary_directory() as directory:
            renderer = self.execution_record(directory, {"1": 2})
            result = measurement(directory, "rubicon", 4, 100, 0, workers=2, renderer=renderer)
            self.assertFalse(result["valid"])
            self.assertEqual(result["counts"]["correct"], 100)
            self.assertIn("Not every runtime confirmed the required warmup threshold", result["problems"])

    def test_verified_metadata_is_valid_but_changed_configuration_is_rejected(self):
        with temporary_directory() as directory:
            renderer = self.execution_record(directory, {"1": 2, "2": 2})
            valid = measurement(directory, "rubicon", 4, 100, 0, workers=2, renderer=renderer)
            self.assertTrue(valid["valid"], valid["problems"])
            directory.joinpath("effective.toml").write_text("[renderer]\nworkers=1\nmax_in_flight=16\nqueue_capacity=16\n")
            changed = measurement(directory, "rubicon", 4, 100, 0, workers=2, renderer=renderer)
            self.assertFalse(changed["valid"])
            self.assertIn("Effective configuration changed during the run", changed["problems"])

    def test_memory_profile_mismatch_invalidates_correct_responses(self):
        with temporary_directory() as directory:
            renderer = self.execution_record(directory, {"1": 2, "2": 2})
            directory.joinpath("server.stderr.log").write_text("RUBICON_DEMO_V8_SEMI_SPACE_MIB=8\n")
            result = measurement(directory, "rubicon", 4, 100, 0, workers=2, renderer=renderer, semi_space_mib=8)
            self.assertFalse(result["valid"])
            self.assertEqual(result["counts"]["correct"], 100)
            self.assertIn("V8 memory profile does not match the planned variant", result["problems"])

    def test_transport_variants_are_grouped_separately(self):
        with temporary_directory() as directory:
            text = self.record(directory)
            binary = copy.deepcopy(text)
            binary["body_transport"] = "bytes"
            groups = group_results([text, binary])
            self.assertEqual(len(groups), 2)
            self.assertEqual({g["body_transport"] for g in groups}, {"text", "bytes"})

    def test_transport_requires_matching_metadata_and_confirmation(self):
        with temporary_directory() as directory:
            renderer = self.execution_record(directory, {"1": 2, "2": 2})
            log = directory / "server.stderr.log"
            log.write_text("RUBICON_DEMO_BODY_TRANSPORT=bytes\n")
            result = measurement(directory, "rubicon", 4, 100, 0, workers=2, renderer=renderer, body_transport="bytes")
            self.assertFalse(result["valid"])

            self.assertIn("Body transport does not match the planned variant", result["problems"])
            path = directory / "run.json"
            metadata = json.loads(path.read_text())
            metadata["body_transport"] = "bytes"
            path.write_text(json.dumps(metadata))
            result = measurement(directory, "rubicon", 4, 100, 0, workers=2, renderer=renderer, body_transport="bytes")
            self.assertTrue(result["valid"], result["problems"])
            log.write_text("")
            result = measurement(directory, "rubicon", 4, 100, 0, workers=2, renderer=renderer, body_transport="bytes")
            self.assertFalse(result["valid"])

    def test_http_profiles_keep_requested_and_effective_counts_separate(self):
        with temporary_directory() as directory:
            first = self.record(directory)
            first.update(http_threads=0, http_threads_actual=16)
            second = copy.deepcopy(first)
            second.update(http_threads=2, http_threads_actual=2)
            third = copy.deepcopy(first)
            third["http_threads_actual"] = 8
            groups = group_results([first, second, third])
            self.assertEqual(len(groups), 3)
            self.assertEqual({(g["http_threads"], g["http_threads_actual"]) for g in groups}, {(0, 16), (2, 2), (0, 8)})

    def test_http_actual_count_and_confirmation_are_required(self):
        with temporary_directory() as directory:
            renderer = self.execution_record(directory, {"1": 2, "2": 2})
            path = directory / "run.json"
            meta = json.loads(path.read_text())
            meta.update(http_threads=2, http_threads_actual=2)
            path.write_text(json.dumps(meta))
            log = directory / "server.stderr.log"
            log.write_text("RUBICON_DEMO_HTTP_THREADS=2\n")
            result = measurement(directory, "rubicon", 4, 100, 0, workers=2, renderer=renderer, http_threads=2)
            self.assertTrue(result["valid"], result["problems"])
            meta["http_threads_actual"] = 4
            path.write_text(json.dumps(meta))
            log.write_text("RUBICON_DEMO_HTTP_THREADS=4\n")
            result = measurement(directory, "rubicon", 4, 100, 0, workers=2, renderer=renderer, http_threads=2)
            self.assertFalse(result["valid"])
            self.assertIn("Effective HTTP thread count differs from the requested profile", result["problems"])


if __name__ == "__main__":
    unittest.main()
