# /// script
# requires-python = ">=3.11"
# dependencies = ["matplotlib>=3.8,<4", "plotly>=6.5,<7"]
# ///
"""Run Criterion comparisons and plot the measured means and confidence intervals."""

import argparse
from datetime import datetime, timezone
import hashlib
import html
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
CASES = {
    "random-u8": "Seeded random u8 · n = 8,192",
    "trailing-one": "Zeros with a trailing one · n = 4,196",
    "leading-one": "Zeros with a leading one · n = 4,196",
    "random-u64": "Seeded random u64 · n = 8,192",
}
PARTITION_ALGORITHMS = (
    "kth", "order-stat", "pdqselect", "adqselect", "tpt-simd-sort",
    "std", "turboselect", "sort", "sort_unstable",
)
MEDIAN_ALGORITHMS = (*PARTITION_ALGORITHMS, "medians")
MEDIAN_SUITES = {"median-only"}
# Exclude RNG-inclusive workloads and internal microbenchmarks.
MAIN_FILTER = r"^(partition/|median-only/|scaling-partition/|scaling-median/)"
SUITES = {
    "partition": ("Partition at the median", "partition"),
    "median-only": ("Retrieve the median", "median-only"),
}
DEFAULT_SIZES = (16, 64, 256, 1024, 4096, 16384, 65536)
SCALING_CASES = {
    "random-u8": "Seeded random u8",
    "trailing-one": "Zeros with a trailing one",
    "leading-one": "Zeros with a leading one",
    "random-u64": "Seeded random u64",
}
STRING_CASES = {
    "random": "seeded random hex strings",
    "prefix64": "64-byte common prefix",
    "prefix256": "256-byte common prefix",
    "few-unique": "seven distinct strings",
}
SCALING_CASES.update({
    f"{kind}-{case}": f"{label} · {title}"
    for kind, label in (("string", "Owned String"), ("str", "Borrowed str"))
    for case, title in STRING_CASES.items()
})
SCALING_CASES["random-wide64"] = "Seeded random 64-byte values"


def partition_algorithms(case):
    # tpt-simd-sort requires Copy, which owned String does not implement.
    return tuple(name for name in PARTITION_ALGORITHMS
                 if name != "tpt-simd-sort" or not case.startswith("string-"))


COLORS = dict(zip(
    (*PARTITION_ALGORITHMS, "medians"),
    ("#2864dc", "#14836c", "#cc493d", "#8255b5", "#9a6324",
     "#e58c16", "#a39110", "#ca64a1", "#617489", "#1a9bb1"),
))
PROFILES = {
    "full": {"label": "Criterion default sampling", "warmup": "3", "measurement": "5",
             "samples": "100", "resamples": "100000"},
    "quick": {"label": "short validation run", "warmup": "0.1", "measurement": "0.3",
              "samples": "20", "resamples": "1000"},
    "ci": {"label": "CI sampling", "warmup": "0.5", "measurement": "1",
           "samples": "50", "resamples": "10000"},
}


def sampling_flags(profile):
    settings = PROFILES[profile]
    return ["--noplot", "--confidence-level", "0.95", "--warm-up-time", settings["warmup"],
            "--measurement-time", settings["measurement"], "--sample-size", settings["samples"],
            "--nresamples", settings["resamples"]]


def expected_keys(sizes=()):
    keys = {("partition", case, name, None) for case in CASES for name in PARTITION_ALGORITHMS}
    keys |= {("median-only", kind, name, None) for kind in ("u8", "u64")
             for name in MEDIAN_ALGORITHMS}
    keys |= {("scaling-partition", case, name, n) for n in sizes for case in SCALING_CASES
             for name in partition_algorithms(case)}
    keys |= {("scaling-median", kind, name, n | 1) for n in sizes for kind in ("u8", "u64")
             for name in MEDIAN_ALGORITHMS}
    return keys


def benchmark_key(full_id):
    if full_id.startswith("scaling-"):
        suite, case, algorithm, n = full_id.split("/")
        return suite, case, algorithm, int(n)
    suite, case, algorithm = full_id.split("/")
    return suite, case, algorithm, None


def load_results(raw, sizes=()):
    rows = []
    found = set()
    expected = expected_keys(sizes)
    for path in sorted(raw.glob("**/new/benchmark.json")):
        identity = json.loads(path.read_text())
        key = benchmark_key(identity["full_id"])
        if key not in expected:
            raise ValueError(f"Unexpected benchmark: {identity['full_id']}")
        if key in found:
            raise ValueError(f"Duplicate benchmark result: {key}")
        found.add(key)
        estimate = json.loads(path.with_name("estimates.json").read_text())["mean"]
        ci = estimate["confidence_interval"]
        mean, low, high = (estimate["point_estimate"], ci["lower_bound"], ci["upper_bound"])
        if not all(math.isfinite(x) and x > 0 for x in (mean, low, high)):
            raise ValueError(f"Non-positive or non-finite timing for {key}")
        if not low <= mean <= high or not math.isclose(ci["confidence_level"], 0.95):
            raise ValueError(f"Invalid 95% confidence interval for {key}")
        rows.append(dict(zip(("suite", "case", "algorithm", "n"), key),
                         mean_ns=mean, lower_ns=low, upper_ns=high,
                         sample_count=len(json.loads(path.with_name("sample.json").read_text())["times"])))
    missing = expected - found
    if missing:
        raise ValueError(f"Missing benchmark results: {sorted(missing)}")
    return rows


def run(command, **kwargs):
    print("+", " ".join(map(str, command)), flush=True)
    subprocess.run(command, cwd=ROOT, check=True, **kwargs)


def capture(command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def provenance(profile, raw, sizes):
    locks = {}
    for name, path in (("benchmarks", ROOT / "benchmarks/Cargo.lock"),):
        data = path.read_bytes()
        locks[name] = {"sha256": hashlib.sha256(data).hexdigest(),
                       "packages": tomllib.loads(data.decode())["package"]}
    try:
        commit = capture(["git", "rev-parse", "HEAD"])
        dirty = bool(capture(["git", "status", "--porcelain"]))
    except subprocess.CalledProcessError:
        commit, dirty = None, None
    cpu = platform.processor()
    if Path("/proc/cpuinfo").exists():
        cpu = next((line.split(":", 1)[1].strip()
                    for line in Path("/proc/cpuinfo").read_text().splitlines()
                    if line.startswith("model name")), cpu)
    return {"generated_at": datetime.now(timezone.utc).isoformat(),
            "profile": PROFILES[profile]["label"], "sampling": PROFILES[profile],
            "rustc": capture(["rustc", "-Vv"]), "platform": platform.platform(),
            "cpu": cpu, "logical_cpus": os.cpu_count(), "git_commit": commit,
            "working_tree_dirty": dirty, "rustflags": os.environ.get("RUSTFLAGS", ""),
            "raw_directory": str(raw), "dependencies": locks,
            "input_seed": 42, "string_and_wide_input_count": 16,
            "scaling_sizes": sizes, "median_scaling_sizes": sorted({n | 1 for n in sizes}),
            "actions_run_url": (f"{os.environ.get('GITHUB_SERVER_URL', 'https://github.com')}/"
                                f"{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}"
                                if os.environ.get("GITHUB_RUN_ID") and os.environ.get("GITHUB_REPOSITORY") else None)}


def measure(output, profile, sizes):
    (output / "raw").mkdir(parents=True, exist_ok=True)
    # A fresh directory prevents old or partial runs from silently entering a chart.
    raw = Path(tempfile.mkdtemp(prefix="run-", dir=output / "raw"))
    flags = sampling_flags(profile)
    env = dict(os.environ, CRITERION_HOME=str(raw), KTH_BENCH_SIZES=",".join(map(str, sizes)))
    command = ["cargo", "bench", "--manifest-path", "benchmarks/Cargo.toml",
               "--bench", "comparisons", "--", MAIN_FILTER]
    run(command + flags, env=env)
    return {"schema_version": 3, "metadata": provenance(profile, raw, sizes),
            "results": load_results(raw, sizes)}


def render_scaling(rows, output, short, linear):
    import matplotlib.pyplot as plt
    from matplotlib.ticker import FuncFormatter
    import interactive_charts

    sections = []
    for suite, label, cases in (
        ("scaling-partition", "Partition at the median", SCALING_CASES),
        ("scaling-median", "Median retrieval", {"u8": "Seeded random u8", "u64": "Seeded random u64"}),
    ):
        for case, title in cases.items():
            selected = [r for r in rows if r["suite"] == suite and r["case"] == case]
            if not selected:
                continue
            figure, axis = plt.subplots(figsize=(11, 6.4))
            figure.patch.set_facecolor("#f6f8fb")
            algorithms = [name for name in COLORS if any(r["algorithm"] == name for r in selected)]
            for name in algorithms:
                points = sorted((r for r in selected if r["algorithm"] == name), key=lambda r: r["n"])
                x = [r["n"] for r in points]
                y = [r["mean_ns"] / 1000 for r in points]
                low = [r["lower_ns"] / 1000 for r in points]
                high = [r["upper_ns"] / 1000 for r in points]
                legend = name + (" (full sort)" if name.startswith("sort") else "")
                axis.plot(x, y, label=legend, color=COLORS[name], marker="o", markersize=4,
                          linewidth=2 if name == "kth" else 1.5,
                          linestyle="--" if name.startswith("sort") else "-")
                axis.fill_between(x, low, high, color=COLORS[name], alpha=0.13)
            if not linear:
                axis.set_xscale("log", base=2)
                axis.set_yscale("log")
            ticks = sorted({r["n"] for r in selected})
            axis.set_xticks(ticks)
            axis.xaxis.set_major_formatter(FuncFormatter(lambda value, _: f"{value:,.0f}"))
            axis.yaxis.set_major_formatter(FuncFormatter(lambda value, _: f"{value:g}"))
            axis.set_xlabel("Number of elements n" + (" (log scale)" if not linear else ""))
            axis.set_ylabel("Mean time per operation (µs)" + (" (log scale)" if not linear else ""))
            axis.set_title(title + (" · SHORT RUN" if short else ""), loc="left", fontsize=15,
                           fontweight="bold", y=1.0, pad=12)
            axis.grid(which="major", color="#dce3ed", linewidth=0.7)
            axis.grid(which="minor", color="#eaf0f6", linewidth=0.4)
            axis.spines[["top", "right"]].set_visible(False)
            axis.legend(loc="upper left", bbox_to_anchor=(0, -0.18), ncol=4, frameon=False, fontsize=9)
            figure.text(0.03, 0.02, f"{label} · lower is faster · shaded bands: 95% confidence intervals\n"
                        "Matched inputs per n; cloning and validation excluded. Points are measurements; lines connect them.",
                        fontsize=9, color="#465569")
            # Reserve fixed space for titles and the two-row legend on log axes.
            figure.subplots_adjust(left=0.12, right=0.98, top=0.90, bottom=0.28)
            filename = f"{suite}-{case}"
            for extension in ("svg", "png"):
                figure.savefig(output / f"{filename}.{extension}", dpi=150, facecolor=figure.get_facecolor())
            plt.close(figure)
            interactive = interactive_charts.scaling_figure(selected, title, COLORS, short, linear)
            sections.append(f'<section><h3>{html.escape(label + " — " + title)}</h3>' +
                            interactive_charts.chart_html(interactive, filename) +
                            f'<p><a href="{filename}.svg">Download SVG</a> · '
                            f'<a href="{filename}.png">Download PNG</a></p></section>')
    return ('<section id="scaling"><h2>Performance by input size</h2><p>One line per implementation. '
            'Median-only inputs use the next odd length when needed; the x axis shows the actual length. '
            'Choose a baseline to compare time multipliers at each input size.</p>' + "".join(sections) + '</section>') if sections else ""


def render(report, output, linear=False):
    if report.get("schema_version") != 3:
        raise ValueError("This report predates the unified benchmark crate; rerun without --render-only. "
                         "Measurements from separate harnesses cannot be combined.")
    # Keep parsing/tests usable with Python's standard library alone.
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.ticker import MaxNLocator
    import interactive_charts
    import plotly

    metadata = report["metadata"]
    metadata["plotting"] = {"python": platform.python_version(), "matplotlib": matplotlib.__version__,
                            "plotly": plotly.__version__}
    interactive_charts.write_assets(output)
    rows = report["results"]
    short = metadata["profile"] == "short validation run"
    sections = [render_scaling(rows, output, short, linear)]
    for suite, (title, filename) in SUITES.items():
        if not any(r["suite"] == suite for r in rows):
            continue
        cases = ("u8", "u64") if suite in MEDIAN_SUITES else tuple(CASES)
        figure, axes = plt.subplots(len(cases), 1, figsize=(11, 3.9 * len(cases)))
        figure.patch.set_facecolor("#f6f8fb")
        for axis, case in zip(axes, cases):
            selected = sorted((r for r in rows if r["suite"] == suite and r["case"] == case),
                              key=lambda row: row["mean_ns"])
            baseline = next(r["mean_ns"] for r in selected if r["algorithm"] == "kth")
            means = [r["mean_ns"] / 1000 for r in selected]
            error = [[(r["mean_ns"] - r["lower_ns"]) / 1000 for r in selected],
                     [(r["upper_ns"] - r["mean_ns"]) / 1000 for r in selected]]
            colors = ["#2864dc" if r["algorithm"] == "kth" else "#71859f" for r in selected]
            axis.barh(range(len(selected)), means, xerr=error, height=0.58,
                      color=colors, error_kw={"ecolor": "#172941", "capsize": 3, "linewidth": 1})
            labels = [r["algorithm"] + (" (full sort)" if r["algorithm"].startswith("sort") else "")
                      for r in selected]
            axis.set_yticks(range(len(selected)), labels)
            axis.invert_yaxis()
            limit = max(r["upper_ns"] / 1000 for r in selected)
            for index, row in enumerate(selected):
                axis.text(row["upper_ns"] / 1000 + limit * 0.02, index,
                          f"{row['mean_ns'] / 1000:.3g} µs  ·  {row['mean_ns'] / baseline:.2f}× kth",
                          va="center", fontsize=9, color="#172941")
            axis.set_xlim(0, limit * 1.55)
            axis.set_xlabel("Mean time per operation (µs) — lower is faster", fontsize=9)
            axis.xaxis.set_major_locator(MaxNLocator(6))
            axis.set_axisbelow(True)
            axis.grid(axis="x", color="#e0e6ef", linewidth=0.7)
            axis.spines[["top", "right", "left"]].set_visible(False)
            axis.tick_params(axis="y", length=0)
            subtitle = f"Seeded random {case} · n = 8,193" if suite in MEDIAN_SUITES else CASES[case]
            axis.set_title(subtitle, loc="left", fontsize=12, fontweight="bold", pad=12)
        suffix = " · SHORT RUN" if short else ""
        figure.suptitle(title + suffix, fontsize=17, fontweight="bold", x=0.04, ha="left")
        figure.text(0.04, 0.015, "Bars: mean · whiskers: 95% confidence interval · blue: kth\n"
                    "Ratios use kth in the same panel. Results describe this machine and these inputs.",
                    fontsize=9, color="#465569")
        figure.tight_layout(rect=(0.02, 0.055, 0.99, 0.95), h_pad=2.5)
        for extension in ("svg", "png"):
            figure.savefig(output / f"{filename}.{extension}", dpi=150, facecolor=figure.get_facecolor())
        plt.close(figure)
        case_titles = ({kind: f"Seeded random {kind} · n = 8,193" for kind in cases}
                       if suite in MEDIAN_SUITES else CASES)
        interactive = interactive_charts.fixed_figure([r for r in rows if r["suite"] == suite],
                                                      title, case_titles, COLORS, short)
        table_rows = []
        for row in rows:
            if row["suite"] != suite:
                continue
            table_rows.append("<tr>" + "".join(f"<td>{html.escape(str(value))}</td>" for value in (
                row["case"], row["algorithm"], f"{row['mean_ns'] / 1000:.6g}",
                f"{row['lower_ns'] / 1000:.6g} – {row['upper_ns'] / 1000:.6g}", row["sample_count"])) + "</tr>")
        sections.append(f'<section id="{suite}"><h2>{title}</h2>' +
                        interactive_charts.chart_html(interactive, filename) +
                        f'<p><a href="{filename}.svg">Download SVG</a> · '
                        f'<a href="{filename}.png">Download PNG</a></p>'
                        '<details><summary>Timing data</summary><table><thead><tr>'
                        '<th>Dataset</th><th>Option</th><th>Mean (µs)</th><th>95% CI (µs)</th>'
                        '<th>Samples</th></tr></thead><tbody>' + "".join(table_rows) + '</tbody></table></details></section>')
    warning = ("Short validation run: use the default command for longer sampling before drawing conclusions."
               if short else "CI sampling on a shared GitHub runner; measurements can vary between runs."
               if metadata["profile"] == "CI sampling" else
               "Measurements are specific to this machine, compiler, and input distribution.")
    run_link = (f'<p>Generated {html.escape(metadata["generated_at"])} · '
                f'<a href="{html.escape(metadata["actions_run_url"], quote=True)}">GitHub Actions run</a></p>'
                if metadata.get("actions_run_url") else "")
    document = """<!doctype html><html lang="en"><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><title>kth benchmark comparisons</title>
<script src="plotly.min.js"></script>
<style>body{margin:2rem auto;padding:0 1.5rem;max-width:1120px;font:16px/1.5 system-ui;color:#172941;background:#f6f8fb}
nav a{display:inline-block;margin:0 1rem .5rem 0}a{color:#1854c1}
section{margin:3rem 0}table{border-collapse:collapse;width:100%;font-variant-numeric:tabular-nums}
th,td{padding:.5rem;text-align:left;border-bottom:1px solid #d5deeb}pre{white-space:pre-wrap;overflow-wrap:anywhere}
.notice{padding:1rem;background:#fff1d6;border-radius:8px}</style>
<h1>kth benchmark comparisons</h1><p class="notice">WARNING_TEXT</p>
RUN_LINK
<p>Hover for times and confidence intervals. Drag to zoom, use the toolbar to pan or reset,
and click an implementation in the legend to hide it (double-click to isolate it).
Confidence bands toggle with their lines. Use the chart buttons to switch log/linear axes.
Size charts also offer absolute time or a time multiplier relative to any available implementation.
The baseline is matched at each input size; 1× is the baseline and lower is faster.
The toolbar exports the current view as SVG; PNG/SVG download links retain the original static views.</p>
<noscript><p>Enable JavaScript for interactive charts. Static PNG/SVG charts remain available through the download links.</p></noscript>
<p>Fixed inputs are identical within each panel; cloning and validation are outside timing.
Partition charts include full-sort baselines, labeled as such. Median-only charts compare retrieval
and do not require partitioned output. All libraries run together in one unpublished benchmark crate.
String and 64-byte scaling cases cycle through sixteen seeded inputs per size.
Owned String panels omit tpt-simd-sort because it requires Copy; borrowed str panels include it.
Fixed-case ratios divide mean time by kth's mean in the same panel. Scaling ratios use the selected
baseline at each input size; below 1× means faster than that baseline.
Whiskers describe individual means, not uncertainty in the ratios.
In multiplier views, confidence bands are divided by the baseline mean and do not include baseline uncertainty.</p>
<nav><a href="#scaling">Performance by input size</a><a href="#partition">Partition</a><a href="#median-only">Median only</a></nav>
SECTIONS
<h2>Run details</h2><p><a href="results.json">Measurements and provenance (JSON)</a></p>
<details><summary>Compiler, machine, dependency versions, and sampling profile</summary><pre>METADATA</pre></details>
</html>"""
    document = document.replace("WARNING_TEXT", html.escape(warning)).replace("SECTIONS", "".join(sections))
    document = document.replace("RUN_LINK", run_link)
    document = document.replace("METADATA", html.escape(json.dumps(metadata, indent=2)))
    (output / "index.html").write_text(document)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/benchmark-report")
    profiles = parser.add_mutually_exclusive_group()
    profiles.add_argument("--quick", action="store_true", help="Short run to validate the report, with visibly labeled charts")
    profiles.add_argument("--ci", action="store_true", help="50 samples with 0.5s warmup and 1s measurement per benchmark")
    parser.add_argument("--render-only", action="store_true", help="Regenerate charts from OUTPUT/results.json without benchmarking")
    parser.add_argument("--sizes", type=int, nargs="+", default=DEFAULT_SIZES,
                        help="Positive input lengths for the scaling charts (default: 16 through 65536)")
    parser.add_argument("--linear", action="store_true", help="Use linear axes for the size charts instead of log/log")
    args = parser.parse_args()
    sizes = sorted(set(args.sizes))
    if any(n <= 0 for n in sizes) or len({n | 1 for n in sizes}) < 2:
        parser.error("choose at least two positive sizes with different odd median lengths")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if args.render_only:
        report = json.loads((output / "results.json").read_text())
    else:
        report = measure(output, "quick" if args.quick else "ci" if args.ci else "full", sizes)
        # Retain completed measurements even if plotting fails.
        (output / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    render(report, output, args.linear)
    (output / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"Report: {output / 'index.html'}", flush=True)


if __name__ == "__main__":
    main()
