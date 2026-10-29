"""Keep operation counts matched to timings and unavailable counters explicit."""
import csv
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import selection_report as report


class SelectionReportTest(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        self.counts = self.root / "counts.csv"
        self.names = [f"selection/u64/random/median/{algorithm}/65" for algorithm in ("kth", "std")]
        for i, name in enumerate(self.names):
            path = self.root / str(i) / "new"
            path.mkdir(parents=True)
            (path / "benchmark.json").write_text(json.dumps({"full_id": name}))
            (path / "estimates.json").write_text(json.dumps({"mean": {
                "point_estimate": 2000, "confidence_interval": {
                    "lower_bound": 1900, "upper_bound": 2100, "confidence_level": 0.95}}}))
            (path / "sample.json").write_text(json.dumps({"times": [2000] * 20}))
        self.raw_counts = [dict(benchmark=name, k=32, element_bytes=8, array_bytes=520, calls=2,
                                comparisons=260, sampling_comparisons=20, partition_comparisons=140,
                                fallback_comparisons=100, elements_scanned=390, bytes_moved=1040,
                                partition_passes=4, small_slice_dispatches=2, budget_fallbacks=0,
                                budget_fallback_rate=0) for name in self.names]
        for key in (*report.INTERNAL, "budget_fallback_rate"):
            self.raw_counts[1][key] = ""
        self.write_counts()

    def write_counts(self):
        with self.counts.open("w", newline="") as stream:
            writer = csv.DictWriter(stream, fieldnames=list(self.raw_counts[0]))
            writer.writeheader()
            writer.writerows(self.raw_counts)

    def test_counts_join_exact_ids_and_normalize_totals(self):
        rows = report.merge_counts(report.load_timings(self.root), self.counts)
        kth, standard = rows
        self.assertEqual(kth["comparisons_per_element"], 2)
        self.assertEqual(kth["scans_per_element"], 3)
        self.assertEqual(kth["bytes_moved_per_element"], 8)
        self.assertEqual(kth["sample_count"], 20)
        self.assertIsNone(standard["elements_scanned"])
        self.assertIsNone(standard["bytes_moved_per_element"])
        self.assertIsNone(standard["budget_fallback_rate"])

    def test_missing_counts_cannot_be_silently_graphed(self):
        self.raw_counts.pop()
        self.write_counts()
        with self.assertRaisesRegex(ValueError, "Missing operation counts"):
            report.merge_counts(report.load_timings(self.root), self.counts)

    def test_duplicate_counts_are_rejected(self):
        self.raw_counts.append(self.raw_counts[0])
        self.write_counts()
        with self.assertRaisesRegex(ValueError, "duplicate"):
            report.merge_counts(report.load_timings(self.root), self.counts)

    def test_phase_counts_must_sum_to_total(self):
        self.raw_counts[0]["sampling_comparisons"] = 21
        self.write_counts()
        with self.assertRaisesRegex(ValueError, "phases"):
            report.merge_counts(report.load_timings(self.root), self.counts)

    def test_std_unavailable_counts_cannot_be_reported_as_zero(self):
        self.raw_counts[1]["bytes_moved"] = 0
        self.write_counts()
        with self.assertRaisesRegex(ValueError, "unavailable"):
            report.merge_counts(report.load_timings(self.root), self.counts)

    def test_opaque_sort_keeps_unknown_movement_and_scans_unavailable(self):
        self.raw_counts[0]["opaque_sort_calls"] = 1
        self.raw_counts[1]["opaque_sort_calls"] = 0
        self.raw_counts[0]["elements_scanned"] = ""
        self.raw_counts[0]["bytes_moved"] = ""
        self.write_counts()
        row = report.merge_counts(report.load_timings(self.root), self.counts)[0]
        self.assertEqual(row["opaque_sort_calls"], 1)
        self.assertEqual(row["comparisons"], 260)
        self.assertIsNone(row["scans_per_element"])
        self.assertIsNone(row["bytes_moved_per_element"])
        self.assertEqual(row["budget_fallback_rate"], 0)
        self.raw_counts[0]["bytes_moved"] = 0
        self.write_counts()
        with self.assertRaisesRegex(ValueError, "unavailable"):
            report.merge_counts(report.load_timings(self.root), self.counts)

    def test_missing_counts_require_an_opaque_sort_marker(self):
        self.raw_counts[0]["elements_scanned"] = ""
        self.write_counts()
        with self.assertRaisesRegex(ValueError, "Missing"):
            report.merge_counts(report.load_timings(self.root), self.counts)

    def test_measure_keeps_counters_out_of_timing_build(self):
        timings = report.load_timings(self.root)
        settings = {"KTH_BENCH_PROFILE": "thresholds", "KTH_BENCH_SIZES": "65"}
        with patch.object(report.common, "run") as run, \
                patch.object(report, "load_timings", return_value=timings), \
                patch.object(report, "merge_counts", return_value=[dict(n=65)]), \
                patch.object(report.common, "provenance", return_value={"string_and_wide_input_count": 16,
                             "scaling_sizes": [65], "median_scaling_sizes": [65]}), \
                patch.dict(report.os.environ, {"KTH_BENCH_FILTER": "stale", "KTH_BENCH_IDS": "stale"}):
            report.measure(self.root, "quick", settings, "/random/")
        timing, counting = run.call_args_list
        self.assertNotIn("--features", timing.args[0])
        self.assertIn("instrumentation", counting.args[0])
        self.assertNotIn("KTH_BENCH_FILTER", timing.kwargs["env"])
        self.assertNotIn("KTH_BENCH_IDS", timing.kwargs["env"])
        self.assertEqual(set(Path(counting.kwargs["env"]["KTH_BENCH_IDS"]).read_text().splitlines()), set(self.names))

    def test_source_changes_prevent_joining_different_implementations(self):
        with patch.object(report, "source_hashes", side_effect=[{"source": "before"}, {"source": "after"}]), \
                patch.object(report.common, "run") as run:
            with self.assertRaisesRegex(ValueError, "sources changed"):
                report.measure(self.root, "quick", {"KTH_BENCH_PROFILE": "ranks"}, "/random/")
        self.assertEqual(run.call_count, 1)

    def test_dropdown_metrics_keep_series_in_the_same_order(self):
        try:
            import plotly.graph_objects as go
        except ImportError:
            self.skipTest("Plotly not installed")
        rows = report.merge_counts(report.load_timings(self.root), self.counts)
        figures = []

        def capture(figure, **kwargs):
            figures.append(figure.to_plotly_json())
            return "<div>chart</div>"

        for profile, x in (("ranks", 0.5), ("thresholds", 65), ("cache", 520)):
            data = {"results": list(reversed(rows)), "metadata": {"profile": "short validation run",
                    "focused_settings": {"KTH_BENCH_PROFILE": profile}}}
            with patch.object(go.Figure, "to_html", autospec=True, side_effect=capture):
                report.render(data, self.root)
            figure = figures[-1]
            self.assertEqual([trace["x"] for trace in figure["data"]], [[x], [x]])
            buttons = {b["label"]: b["args"] for b in figure["layout"]["updatemenus"][0]["buttons"]}
            self.assertEqual(buttons["Logical bytes moved / element"][0]["y"], [[8], [None]])
            self.assertEqual(buttons["Budget fallback frequency"][0]["y"], [[0], [None]])
            self.assertFalse(buttons["Budget fallback frequency"][0]["error_y.visible"])
            self.assertTrue(buttons["Time (µs)"][0]["error_y.visible"])
        self.assertTrue((self.root / "plotly.min.js").exists())


if __name__ == "__main__":
    unittest.main()
