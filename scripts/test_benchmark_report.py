"""Validate Criterion's file format and completeness before graphing any results."""

import json
from pathlib import Path
import re
import tempfile
import unittest

import benchmark_report as report


class CriterionResultsTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.raw = Path(self.directory.name)
        self.paths = []
        self.sizes = (16, 64)
        for index, (suite, case, algorithm, n) in enumerate(sorted(report.expected_keys(self.sizes))):
            if suite.startswith("scaling-"):
                name = f"{suite}/{case}/{algorithm}/{n}"
            elif suite == "experimental":
                name = f"turboselect-comparison/{algorithm}/{case}"
            elif suite in report.MEDIAN_SUITES:
                name = f"{suite}/{case}/{algorithm}"
            else:
                name = f"{algorithm} {case}"
            path = self.raw / str(index) / "new"
            path.mkdir(parents=True)
            (path / "benchmark.json").write_text(json.dumps({"full_id": name}))
            (path / "estimates.json").write_text(json.dumps({"mean": {
                "point_estimate": 2000.0,
                "confidence_interval": {"confidence_level": 0.95,
                                        "lower_bound": 1900.0, "upper_bound": 2100.0}}}))
            (path / "sample.json").write_text(json.dumps({"times": [4000.0] * 20}))
            self.paths.append(path)

    def test_loads_complete_run_without_merging_harness_baselines(self):
        rows = report.load_results(self.raw, self.sizes)
        self.assertEqual(len(rows), 177)
        baselines = [r for r in rows if r["algorithm"] == "kth" and r["case"] == "rand fixed"]
        self.assertEqual({r["suite"] for r in baselines}, {"partition", "experimental"})
        self.assertEqual(rows[0]["mean_ns"], 2000.0)
        self.assertEqual(rows[0]["lower_ns"], 1900.0)
        self.assertEqual(rows[0]["upper_ns"], 2100.0)
        self.assertEqual(rows[0]["sample_count"], 20)

    def test_scaling_keeps_actual_element_counts(self):
        rows = report.load_results(self.raw, self.sizes)
        medians = [r for r in rows if r["suite"] == "scaling-median"]
        self.assertEqual({r["n"] for r in medians}, {17, 65})
        partitions = [r for r in rows if r["suite"] == "scaling-partition"]
        self.assertEqual({r["n"] for r in partitions}, {16, 64})

    def test_retrieval_includes_partition_selectors_for_both_types(self):
        rows = report.load_results(self.raw, self.sizes)
        names = {"kth", "std", "order-stat", "pdqselect", "adqselect", "tpt-simd-sort",
                 "sort", "sort_unstable", "medians"}
        for suite in ("median-only", "scaling-median"):
            for kind in ("u8", "u64"):
                self.assertEqual({r["algorithm"] for r in rows if r["suite"] == suite and r["case"] == kind}, names)
        for suite in ("median-only-experimental", "scaling-median-experimental"):
            for kind in ("u8", "u64"):
                self.assertEqual({r["algorithm"] for r in rows if r["suite"] == suite and r["case"] == kind},
                                 {"kth", "std", "turboselect", "medians"})

    def test_incomplete_scaling_series_cannot_be_graphed(self):
        path = next(p for p in self.paths if "scaling-partition" in (p / "benchmark.json").read_text())
        (path / "benchmark.json").unlink()
        with self.assertRaisesRegex(ValueError, "Missing benchmark results.*scaling-partition"):
            report.load_results(self.raw, self.sizes)

    def test_ignores_stale_baseline_files_and_rejects_partial_run(self):
        self.paths[0].rename(self.paths[0].with_name("base"))
        with self.assertRaisesRegex(ValueError, "Missing benchmark results"):
            report.load_results(self.raw, self.sizes)

    def test_rejects_duplicate_benchmarks(self):
        (self.paths[1] / "benchmark.json").write_bytes((self.paths[0] / "benchmark.json").read_bytes())
        with self.assertRaisesRegex(ValueError, "Duplicate benchmark"):
            report.load_results(self.raw, self.sizes)

    def test_rejects_non_finite_timings(self):
        path = self.paths[0] / "estimates.json"
        estimate = json.loads(path.read_text())
        estimate["mean"]["point_estimate"] = float("nan")
        path.write_text(json.dumps(estimate))
        with self.assertRaisesRegex(ValueError, "non-finite"):
            report.load_results(self.raw, self.sizes)

    def test_rejects_mislabeled_confidence_intervals(self):
        path = self.paths[0] / "estimates.json"
        estimate = json.loads(path.read_text())
        estimate["mean"]["confidence_interval"]["confidence_level"] = 0.90
        path.write_text(json.dumps(estimate))
        with self.assertRaisesRegex(ValueError, "95% confidence interval"):
            report.load_results(self.raw, self.sizes)

    def test_filter_excludes_rng_and_internal_workloads(self):
        for name in ("kth rand fixed", "std [0]*4195+[1]", "adqselect [1]+[0]*4195",
                     "median-only/u64/medians"):
            self.assertRegex(name, report.MAIN_FILTER)
        for name in ("std rand", "kth rand", "partition5", "repeated_step3_big"):
            self.assertIsNone(re.search(report.MAIN_FILTER, name))


if __name__ == "__main__":
    unittest.main()
