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


if __name__ == "__main__":
    unittest.main()
