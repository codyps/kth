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
        self.coverage_sizes = (16, 65)
        for index, (suite, case, algorithm, n) in enumerate(sorted(report.expected_keys(self.sizes, self.coverage_sizes))):
            if suite.startswith("scaling-") or suite == "coverage-partition":
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
        rows = report.load_results(self.raw, self.sizes, self.coverage_sizes)
        self.assertEqual(len(rows), len(report.expected_keys(self.sizes, self.coverage_sizes)))
        baselines = [r for r in rows if r["algorithm"] == "kth" and r["case"] == "random-u8"]
        self.assertEqual({r["suite"] for r in baselines}, {"partition", "scaling-partition"})
        self.assertEqual(rows[0]["mean_ns"], 2000.0)
        self.assertEqual(rows[0]["lower_ns"], 1900.0)
        self.assertEqual(rows[0]["upper_ns"], 2100.0)
        self.assertEqual(rows[0]["sample_count"], 20)

    def test_scaling_keeps_actual_element_counts(self):
        rows = report.load_results(self.raw, self.sizes, self.coverage_sizes)
        medians = [r for r in rows if r["suite"] == "scaling-median"]
        self.assertEqual({r["n"] for r in medians}, {17, 65})
        partitions = [r for r in rows if r["suite"] == "scaling-partition"]
        self.assertEqual({r["n"] for r in partitions}, {16, 64})

    def test_retrieval_includes_partition_selectors_for_both_types(self):
        rows = report.load_results(self.raw, self.sizes, self.coverage_sizes)
        names = {"kth", "std", "order-stat", "pdqselect", "adqselect", "tpt-simd-sort",
                 "sort", "sort_unstable", "medians", "turboselect"}
        for suite in ("median-only", "scaling-median"):
            for kind in ("u8", "u64"):
                expected = names | ({"kth-u8"} if kind == "u8" else set())
                self.assertEqual({r["algorithm"] for r in rows if r["suite"] == suite and r["case"] == kind}, expected)

    def test_all_partition_cases_use_the_same_selectors(self):
        rows = report.load_results(self.raw, self.sizes, self.coverage_sizes)
        for suite in ("partition", "scaling-partition"):
            cases = report.CASES if suite == "partition" else report.SCALING_CASES
            for case in cases:
                self.assertEqual({r["algorithm"] for r in rows if r["suite"] == suite and r["case"] == case},
                                 set(report.partition_algorithms(case)))

    def test_report_requires_owned_and_borrowed_string_comparisons(self):
        keys = report.expected_keys(self.sizes, self.coverage_sizes)
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
        self.assertGreater(len(report.expected_keys(report.DEFAULT_SIZES)), 2000)

    def test_missing_string_series_cannot_be_silently_omitted(self):
        path = next(p for p in self.paths if json.loads((p / "benchmark.json").read_text())["full_id"] ==
                    "scaling-partition/string-prefix256/kth/64")
        (path / "benchmark.json").unlink()
        with self.assertRaisesRegex(ValueError, "Missing benchmark results.*string-prefix256"):
            report.load_results(self.raw, self.sizes, self.coverage_sizes)

    def test_report_runs_one_crate_and_one_harness(self):
        from unittest.mock import patch
        with patch.object(report, "run") as run, patch.object(report, "provenance", return_value={}), \
                patch.object(report, "load_results", return_value=[]):
            result = report.measure(self.raw, "quick", self.sizes)
        self.assertEqual(run.call_count, 1)
        command = run.call_args.args[0]
        self.assertEqual(command[:8], ["cargo", "bench", "--manifest-path", "benchmarks/Cargo.toml",
                                     "--bench", "comparisons", "--", report.MAIN_FILTER])
        self.assertEqual(result["schema_version"], 4)
        self.assertEqual(run.call_args.kwargs["env"]["KTH_BENCH_SIZES"], "16,64")

    def test_focused_coverage_requires_each_rank_library_and_threshold(self):
        keys = report.expected_keys(self.sizes)
        for kind in ("64-byte", "128-byte"):
            for n in (16383, 16384, 16385, 1048576):
                for name in report.PARTITION_ALGORITHMS:
                    self.assertIn(("coverage-partition", f"{kind}/random/median", name, n), keys)
        for rank in ("p01", "p25", "median", "p75", "p99"):
            self.assertIn(("coverage-partition", f"64-byte/random/{rank}", "std", 65536), keys)
        for case in ("u8/sawtooth/p01", "u8/ordered-desc/p99", "33-byte/periodic17/median",
                     "33-byte/random17/median", "str/few-unique/p01"):
            self.assertIn(("coverage-partition", case, "kth", 16384), keys)
        self.assertIn(("coverage-partition", "u8/sawtooth/p01", "kth-u8", 16384), keys)
        self.assertNotIn(("coverage-partition", "String/few-unique/median", "tpt-simd-sort", 16384), keys)

    def test_missing_focused_rank_cannot_be_silently_omitted(self):
        path = next(p for p in self.paths if json.loads((p / "benchmark.json").read_text())["full_id"] ==
                    "coverage-partition/33-byte/periodic17/median/std/65")
        (path / "benchmark.json").unlink()
        with self.assertRaisesRegex(ValueError, "Missing benchmark results.*33-byte/periodic17/median"):
            report.load_results(self.raw, self.sizes, self.coverage_sizes)

    def test_focused_override_controls_both_harness_and_expected_results(self):
        from unittest.mock import patch
        with patch.object(report, "run") as run, patch.object(report, "provenance", return_value={}), \
                patch.object(report, "load_results", return_value=[]) as load:
            report.measure(self.raw, "quick", self.sizes, (17, 65))
        self.assertEqual(run.call_args.kwargs["env"]["KTH_PAGE_SIZES"], "17,65")
        self.assertEqual(load.call_args.args[2], (17, 65))

    def test_focused_explorer_exports_matched_std_ratios_and_actual_indices(self):
        import csv
        rows = [dict(suite="coverage-partition", case=case, algorithm=name, n=n, mean_ns=mean,
                     lower_ns=mean * 0.9, upper_ns=mean * 1.1, sample_count=20)
                for case in ("u8/random/p01", "u8/random/median", "128-byte/random/median")
                for name, mean in (("kth", 1000), ("std", 2000)) for n in (16383, 16385)]
        markup = report.render_coverage(rows, self.raw, True, False)
        self.assertIn('id="coverage-case"', markup)
        self.assertIn('id="coverage-rank"', markup)
        self.assertIn('src="coverage-u8-random-median.html"', markup)
        self.assertIn('128-byte · Seeded random · Median', markup)
        self.assertIn('Wide-record random medians: n = 16,383, 16,385.', markup)
        self.assertNotIn('1,048,576', markup)
        with (self.raw / 'coverage-u8-random-p01.csv').open() as source:
            data = list(csv.DictReader(source))
        self.assertEqual({int(r["k"]) for r in data}, {163})
        self.assertEqual({float(r["time_over_std"]) for r in data if r["library"] == "kth (generic)"}, {0.5})
        self.assertIn('plotly.min.js', (self.raw / 'coverage-128-byte-random-median.html').read_text())

    def test_old_reports_require_remeasurement_instead_of_merging_harnesses(self):
        with self.assertRaisesRegex(ValueError, "rerun without --render-only"):
            report.render({"schema_version": 2}, self.raw)

    def test_incomplete_scaling_series_cannot_be_graphed(self):
        path = next(p for p in self.paths if "scaling-partition" in (p / "benchmark.json").read_text())
        (path / "benchmark.json").unlink()
        with self.assertRaisesRegex(ValueError, "Missing benchmark results.*scaling-partition"):
            report.load_results(self.raw, self.sizes, self.coverage_sizes)

    def test_ignores_stale_baseline_files_and_rejects_partial_run(self):
        self.paths[0].rename(self.paths[0].with_name("base"))
        with self.assertRaisesRegex(ValueError, "Missing benchmark results"):
            report.load_results(self.raw, self.sizes, self.coverage_sizes)

    def test_rejects_duplicate_benchmarks(self):
        (self.paths[1] / "benchmark.json").write_bytes((self.paths[0] / "benchmark.json").read_bytes())
        with self.assertRaisesRegex(ValueError, "Duplicate benchmark"):
            report.load_results(self.raw, self.sizes, self.coverage_sizes)

    def test_rejects_non_finite_timings(self):
        path = self.paths[0] / "estimates.json"
        estimate = json.loads(path.read_text())
        estimate["mean"]["point_estimate"] = float("nan")
        path.write_text(json.dumps(estimate))
        with self.assertRaisesRegex(ValueError, "non-finite"):
            report.load_results(self.raw, self.sizes, self.coverage_sizes)

    def test_rejects_mislabeled_confidence_intervals(self):
        path = self.paths[0] / "estimates.json"
        estimate = json.loads(path.read_text())
        estimate["mean"]["confidence_interval"]["confidence_level"] = 0.90
        path.write_text(json.dumps(estimate))
        with self.assertRaisesRegex(ValueError, "95% confidence interval"):
            report.load_results(self.raw, self.sizes, self.coverage_sizes)

    def test_filter_excludes_rng_and_internal_workloads(self):
        for name in ("partition/random-u8/kth", "partition/leading-one/std",
                     "partition/random-u64/adqselect", "median-only/u64/medians",
                     "scaling-partition/random-u64/turboselect/16", "scaling-median/u8/turboselect/17",
                     "coverage-partition/33-byte/periodic17/median/std/16384"):
            self.assertRegex(name, report.MAIN_FILTER)
        for name in ("rng-partition/random-u8/std", "rng-partition/random-u8/turboselect",
                     "partition5", "repeated_step3_big", "std rand"):
            self.assertIsNone(re.search(report.MAIN_FILTER, name))


if __name__ == "__main__":
    unittest.main()
