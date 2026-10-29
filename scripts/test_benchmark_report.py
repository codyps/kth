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
            else:
                name = f"{suite}/{case}/{algorithm}"
            path = self.raw / str(index) / "new"
            path.mkdir(parents=True)
            (path / "benchmark.json").write_text(json.dumps({"full_id": name}))
            (path / "estimates.json").write_text(json.dumps({"mean": {
                "point_estimate": 2000.0,
                "confidence_interval": {"confidence_level": 0.95,
                                        "lower_bound": 1900.0, "upper_bound": 2100.0}}}))
            (path / "sample.json").write_text(json.dumps({"times": [4000.0] * 20}))
            self.paths.append(path)

    def test_loads_complete_unified_run(self):
        rows = report.load_results(self.raw, self.sizes)
        self.assertEqual(len(rows), 322)
        baselines = [r for r in rows if r["algorithm"] == "kth" and r["case"] == "random-u8"]
        self.assertEqual({r["suite"] for r in baselines}, {"partition", "scaling-partition"})
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
                 "sort", "sort_unstable", "medians", "turboselect"}
        for suite in ("median-only", "scaling-median"):
            for kind in ("u8", "u64"):
                self.assertEqual({r["algorithm"] for r in rows if r["suite"] == suite and r["case"] == kind}, names)

    def test_all_partition_cases_use_the_same_selectors(self):
        rows = report.load_results(self.raw, self.sizes)
        for suite in ("partition", "scaling-partition"):
            cases = report.CASES if suite == "partition" else report.SCALING_CASES
            for case in cases:
                self.assertEqual({r["algorithm"] for r in rows if r["suite"] == suite and r["case"] == case},
                                 set(report.partition_algorithms(case)))

    def test_report_requires_owned_and_borrowed_string_comparisons(self):
        keys = report.expected_keys(self.sizes)
        for kind in ("string", "str"):
            for case in ("random", "prefix64", "prefix256", "few-unique"):
                for n in self.sizes:
                    for algorithm in ("kth", "std", "order-stat", "pdqselect", "adqselect", "turboselect", "sort", "sort_unstable"):
                        self.assertIn(("scaling-partition", f"{kind}-{case}", algorithm, n), keys)
                    copy_only = ("scaling-partition", f"{kind}-{case}", "tpt-simd-sort", n)
                    self.assertEqual(copy_only in keys, kind == "str")
        for n in self.sizes:
            for algorithm in report.PARTITION_ALGORITHMS:
                self.assertIn(("scaling-partition", "random-wide64", algorithm, n), keys)
        self.assertEqual(len(report.expected_keys(report.DEFAULT_SIZES)), 987)

    def test_missing_string_series_cannot_be_silently_omitted(self):
        path = next(p for p in self.paths if json.loads((p / "benchmark.json").read_text())["full_id"] ==
                    "scaling-partition/string-prefix256/kth/64")
        (path / "benchmark.json").unlink()
        with self.assertRaisesRegex(ValueError, "Missing benchmark results.*string-prefix256"):
            report.load_results(self.raw, self.sizes)

    def test_report_runs_one_crate_and_one_harness(self):
        from unittest.mock import patch
        with patch.object(report, "run") as run, patch.object(report, "provenance", return_value={}), \
                patch.object(report, "load_results", return_value=[]):
            result = report.measure(self.raw, "quick", self.sizes)
        self.assertEqual(run.call_count, 1)
        command = run.call_args.args[0]
        self.assertEqual(command[:8], ["cargo", "bench", "--manifest-path", "benchmarks/Cargo.toml",
                                     "--bench", "comparisons", "--", report.MAIN_FILTER])
        self.assertEqual(result["schema_version"], 3)
        self.assertEqual(run.call_args.kwargs["env"]["KTH_BENCH_SIZES"], "16,64")

    def test_old_reports_require_remeasurement_instead_of_merging_harnesses(self):
        with self.assertRaisesRegex(ValueError, "rerun without --render-only"):
            report.render({"schema_version": 2}, self.raw)

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
        for name in ("partition/random-u8/kth", "partition/leading-one/std",
                     "partition/random-u64/adqselect", "median-only/u64/medians",
                     "scaling-partition/random-u64/turboselect/16", "scaling-median/u8/turboselect/17"):
            self.assertRegex(name, report.MAIN_FILTER)
        for name in ("rng-partition/random-u8/std", "rng-partition/random-u8/turboselect",
                     "partition5", "repeated_step3_big", "std rand"):
            self.assertIsNone(re.search(report.MAIN_FILTER, name))


if __name__ == "__main__":
    unittest.main()
