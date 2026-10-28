"""Protect timing units, independent baselines, and interactive chart controls."""

from pathlib import Path
import tempfile
import unittest

import interactive_charts as charts


COLORS = {"kth": "#2864dc", "std": "#e58c16"}


def measurement(name, n, mean, case="first"):
    return {"algorithm": name, "n": n, "case": case, "mean_ns": mean,
            "lower_ns": mean * 0.9, "upper_ns": mean * 1.2, "sample_count": 50}


class InteractiveChartsTest(unittest.TestCase):
    def setUp(self):
        self.rows = [measurement("kth", 65, 2000), measurement("std", 17, 2000),
                     measurement("kth", 17, 1000), measurement("std", 65, 8000)]

    def test_sorted_actual_lengths_and_timing_intervals_use_microseconds(self):
        figure = charts.scaling_figure(self.rows, "Median", COLORS)
        line = next(t for t in figure.data if t.name == "std")
        self.assertEqual(list(line.x), [17, 65])
        self.assertEqual(list(line.y), [2, 8])
        self.assertEqual(list(line.customdata[0]), [1.8, 2.4, 50, 2])
        self.assertEqual(line.customdata[1][3], 4)
        self.assertEqual(figure.layout.xaxis.type, "log")
        self.assertEqual(figure.layout.yaxis.type, "log")

    def test_each_legend_entry_toggles_its_line_and_both_bounds(self):
        figure = charts.scaling_figure(self.rows, "Median", COLORS)
        self.assertEqual(figure.layout.legend.groupclick, "togglegroup")
        for name in COLORS:
            traces = [t for t in figure.data if t.legendgroup == name]
            self.assertEqual(len(traces), 3)
            self.assertEqual(sum(t.showlegend is True for t in traces), 1)
            self.assertEqual(traces[1].fill, "tonexty")
            self.assertEqual(traces[0].hoverinfo, "skip")

    def test_scale_buttons_reset_zoom_and_switch_both_axes(self):
        figure = charts.scaling_figure(self.rows, "Median", COLORS, linear=True)
        self.assertEqual(figure.layout.xaxis.type, "linear")
        buttons = figure.layout.updatemenus[0].buttons
        for button, kind in zip(buttons, ("log", "linear")):
            changes = button.args[0]
            for axis in ("xaxis", "yaxis"):
                self.assertEqual(changes[f"{axis}.type"], kind)
                self.assertIs(changes[f"{axis}.autorange"], True)

    def test_fixed_cases_have_independent_baselines_and_asymmetric_errors(self):
        rows = [measurement("kth", None, 1000), measurement("std", None, 2000),
                measurement("kth", None, 2000, "second"), measurement("std", None, 8000, "second")]
        figure = charts.fixed_figure(rows, "Fixed", {"first": "First", "second": "Second"}, COLORS)
        std = [t for t in figure.data if t.name == "std"]
        self.assertEqual([t.customdata[0][3] for t in std], [2, 4])
        self.assertEqual(list(std[0].error_x.array), [0.4])
        self.assertEqual(list(std[0].error_x.arrayminus), [0.2])
        self.assertTrue(std[0].showlegend)
        self.assertFalse(std[1].showlegend)
        self.assertEqual(std[0].legendgroup, std[1].legendgroup)

    def test_html_uses_the_local_runtime_and_preserves_interaction_configuration(self):
        figure = charts.scaling_figure(self.rows, "Median", COLORS)
        snippet = charts.chart_html(figure, "scaling-test")
        self.assertIn('id="scaling-test"', snippet)
        self.assertIn("Plotly.newPlot", snippet)
        self.assertIn('"scrollZoom": true', snippet)
        self.assertNotIn('<script src=', snippet)
        with tempfile.TemporaryDirectory() as directory:
            charts.write_assets(Path(directory))
            asset = Path(directory) / "plotly.min.js"
            self.assertGreater(asset.stat().st_size, 1_000_000)
            self.assertIn("plotly.js", asset.read_text()[:1000])


if __name__ == "__main__":
    unittest.main()
