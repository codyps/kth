# /// script
# requires-python = ">=3.11"
# dependencies = ["matplotlib>=3.8,<4", "plotly>=6.5,<7"]
# ///
"""Run Criterion comparisons and plot the measured means and confidence intervals."""

import argparse
import csv
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
MAIN_FILTER = r"^(partition/|median-only/|scaling-partition/|scaling-median/|coverage-partition/)"
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
    generic = tuple(name for name in PARTITION_ALGORITHMS
                    if name != "tpt-simd-sort" or not case.startswith(("string-", "String/")))
    return generic + (("kth-u8",) if case.startswith("u8/") or case in ("random-u8", "trailing-one", "leading-one") else ())


def coverage_cases(sizes=None):
    """Use the same explicit workload matrix as the Criterion harness."""
    cases = {}
    with (ROOT / "benchmarks/page_cases.csv").open() as source:
        for row in csv.DictReader(source):
            for rank in row["ranks"].split(";"):
                case = "/".join((row["type"], row["distribution"], rank))
                lengths = sizes if sizes is not None else map(int, row["sizes"].split(";"))
                cases.setdefault(case, set()).update(lengths)
    return cases


def coverage_title(case):
    kind, distribution, rank = case.split("/")
    kind = {"String": "Owned String", "str": "Borrowed &str"}.get(kind, kind)
    distribution = {"random": "Seeded random", "random17": "17 random distinct values",
                    "periodic17": "Periodic ramp, 17 values", "sawtooth": "Sawtooth, 7 values",
                    "dominant": "90% dominant value", "clustered": "90% equal prefix",
                    "ordered-asc": "Ascending", "ordered-desc": "Descending",
                    "organ-pipe": "Organ pipe", "few-unique": "Seven distinct strings"}[distribution]
    rank = {"p01": "1st percentile", "p25": "25th percentile", "median": "Median",
            "p75": "75th percentile", "p99": "99th percentile"}[rank]
    return f"{kind} · {distribution} · {rank}"


def median_algorithms(kind):
    return MEDIAN_ALGORITHMS + (("kth-u8",) if kind == "u8" else ())


COLORS = dict(zip(
    (*PARTITION_ALGORITHMS, "medians"),
    ("#0072b2", "#009e73", "#e69f00", "#aa4499", "#56b4e9",
     "#333333", "#d55e00", "#888888", "#999933", "#cc79a7"),
))
COLORS["kth-u8"] = "#117733"
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


def expected_keys(sizes=(), coverage_sizes=None):
    keys = {("partition", case, name, None) for case in CASES for name in partition_algorithms(case)}
    keys |= {("median-only", kind, name, None) for kind in ("u8", "u64")
             for name in median_algorithms(kind)}
    keys |= {("scaling-partition", case, name, n) for n in sizes for case in SCALING_CASES
             for name in partition_algorithms(case)}
    keys |= {("scaling-median", kind, name, n | 1) for n in sizes for kind in ("u8", "u64")
             for name in median_algorithms(kind)}
    keys |= {("coverage-partition", case, name, n) for case, lengths in coverage_cases(coverage_sizes).items()
             for n in lengths for name in partition_algorithms(case)}
    return keys


def benchmark_key(full_id):
    if full_id.startswith("coverage-partition/"):
        suite, kind, distribution, rank, algorithm, n = full_id.split("/")
        return suite, "/".join((kind, distribution, rank)), algorithm, int(n)
    if full_id.startswith("scaling-"):
        suite, case, algorithm, n = full_id.split("/")
        return suite, case, algorithm, int(n)
    suite, case, algorithm = full_id.split("/")
    return suite, case, algorithm, None


def load_results(raw, sizes=(), coverage_sizes=None):
    rows = []
    found = set()
    expected = expected_keys(sizes, coverage_sizes)
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


def provenance(profile, raw, sizes, coverage_sizes=None):
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
            "coverage_cases": {case: sorted(lengths) for case, lengths in coverage_cases(coverage_sizes).items()},
            "coverage_manifest_sha256": hashlib.sha256((ROOT / "benchmarks/page_cases.csv").read_bytes()).hexdigest(),
            "coverage_random_inputs": "16 banks; 4 for inline arrays of at least 64 MiB; one bank for structured inputs",
            "actions_run_url": (f"{os.environ.get('GITHUB_SERVER_URL', 'https://github.com')}/"
                                f"{os.environ['GITHUB_REPOSITORY']}/actions/runs/{os.environ['GITHUB_RUN_ID']}"
                                if os.environ.get("GITHUB_RUN_ID") and os.environ.get("GITHUB_REPOSITORY") else None)}


def measure(output, profile, sizes, coverage_sizes=None):
    (output / "raw").mkdir(parents=True, exist_ok=True)
    # A fresh directory prevents old or partial runs from silently entering a chart.
    raw = Path(tempfile.mkdtemp(prefix="run-", dir=output / "raw"))
    flags = sampling_flags(profile)
    env = dict(os.environ, CRITERION_HOME=str(raw), KTH_BENCH_SIZES=",".join(map(str, sizes)))
    # A report's expected keys and its recorded sizes must control the harness,
    # rather than inheriting a hidden shell override.
    env.pop("KTH_PAGE_SIZES", None)
    if coverage_sizes is not None:
        env["KTH_PAGE_SIZES"] = ",".join(map(str, coverage_sizes))
    command = ["cargo", "bench", "--manifest-path", "benchmarks/Cargo.toml",
               "--bench", "comparisons", "--", MAIN_FILTER]
    run(command + flags, env=env)
    return {"schema_version": 4, "metadata": provenance(profile, raw, sizes, coverage_sizes),
            "results": load_results(raw, sizes, coverage_sizes)}


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
            figure, axis = plt.subplots(figsize=(12.8, 6.4))
            figure.patch.set_facecolor("#f6f8fb")
            algorithms = [name for name in COLORS if any(r["algorithm"] == name for r in selected)]
            for name in algorithms:
                points = sorted((r for r in selected if r["algorithm"] == name), key=lambda r: r["n"])
                x = [r["n"] for r in points]
                y = [r["mean_ns"] / 1000 for r in points]
                low = [r["lower_ns"] / 1000 for r in points]
                high = [r["upper_ns"] / 1000 for r in points]
                legend = interactive_charts.label(name)
                markers = dict(zip(interactive_charts.SYMBOLS, ("o", "D", "s", "^", "v", "+", "x", "*", "o", "s", "h")))
                axis.plot(x, y, label=legend, color=COLORS[name], marker=markers[name], markersize=5,
                          linewidth=2 if name == "kth" else 1.5,
                          markerfacecolor="none" if name.startswith("sort") else COLORS[name],
                          linestyle={"std": "-.", "sort": "--", "sort_unstable": ":"}.get(name, "-"))
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
            axis.legend(loc="upper left", bbox_to_anchor=(1.02, 1), frameon=False, fontsize=9,
                        title="Library / API")
            figure.text(0.03, 0.02, f"{label} · lower is faster · shaded bands: 95% confidence intervals\n"
                        "Matched inputs per n; cloning and validation excluded. Points are measurements; lines connect them.",
                        fontsize=9, color="#465569")
            # Reserve fixed space for titles and the two-row legend on log axes.
            figure.subplots_adjust(left=0.10, right=0.70, top=0.90, bottom=0.20)
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


def render_coverage(rows, output, short, linear):
    """Load one focused chart at a time, keeping the expanded page responsive."""
    import interactive_charts

    selected = [r for r in rows if r["suite"] == "coverage-partition"]
    if not selected:
        return ""
    index = []
    for case in dict.fromkeys(r["case"] for r in selected):
        points = [r for r in selected if r["case"] == case]
        kind, distribution, rank = case.split("/")
        name = "coverage-" + case.replace("/", "-")
        title = coverage_title(case)
        figure = interactive_charts.scaling_figure(points, title, COLORS, short, linear)
        snippet = interactive_charts.chart_html(figure, name)
        baseline = {r["n"]: r["mean_ns"] for r in points if r["algorithm"] == "std"}
        with (output / f"{name}.csv").open("w", newline="") as dest:
            fields = ["library", "n", "k", "mean_ns", "lower_ns", "upper_ns", "samples", "time_over_std"]
            writer = csv.DictWriter(dest, fieldnames=fields)
            writer.writeheader()
            for row in points:
                n = row["n"]
                k = n // 2 if rank == "median" else n * int(rank[1:]) // 100
                writer.writerow(dict(library=interactive_charts.label(row["algorithm"]), n=n, k=k,
                                     mean_ns=row["mean_ns"], lower_ns=row["lower_ns"], upper_ns=row["upper_ns"],
                                     samples=row["sample_count"], time_over_std=row["mean_ns"] / baseline[n]))
        (output / f"{name}.html").write_text(
            '<!doctype html><html lang="en"><meta charset="utf-8">'
            '<meta name="viewport" content="width=device-width,initial-scale=1">'
            f'<title>{html.escape(title)}</title><script src="plotly.min.js"></script>'
            '<style>body{margin:0;font:14px/1.5 system-ui;color:#172941;background:#f6f8fb}p{padding:0 1rem}</style>'
            + snippet + f'<p><a href="{name}.csv">Download timing data (CSV)</a>. '
            'Choose Time / std::select_nth_unstable to show the 0.8× guide (20% less elapsed time). '
            'Bands scale individual confidence intervals by a measured baseline mean; they are not ratio intervals.</p></html>')
        index.append(dict(case=f"{kind}/{distribution}", kind=kind, rank=rank, title=title,
                          rank_label=title.split(" · ")[-1], url=f"{name}.html", csv=f"{name}.csv"))
    first_case = index[0]["case"]
    first = next((item for item in index if item["case"] == first_case and item["rank"] == "median"), index[0])
    options = "".join(f'<option value="{html.escape(case, quote=True)}">{html.escape(coverage_title(case + "/median").rsplit(" · ", 1)[0])}</option>'
                      for case in dict.fromkeys(item["case"] for item in index))
    rank_options = "".join(f'<option value="{item["rank"]}"' + (' selected' if item == first else '') +
                           f'>{item["rank_label"]}</option>' for item in index if item["case"] == first_case)
    links = "".join(f'<li><a href="{item["url"]}">{html.escape(item["title"])}</a> · '
                    f'<a href="{item["csv"]}">CSV</a></li>' for item in index)
    data = json.dumps(index).replace("<", "\\u003c")
    return ('<section id="coverage"><h2>Ranks and difficult inputs</h2>'
            f'<p>{len(index)} workload/rank panels · {len(selected):,} measurements. '
            'Compare tail and interior ranks, byte concentration, organ pipes, periodic ramps, '
            'few-unique strings and 17/32/33/64/128-byte records. Random wide-record medians '
            'include both sides of 16,384 and extend to 1,048,576 elements. Each panel shows its actual measured lengths.</p>'
            '<div class="case-controls"><label for="coverage-case">Input <select id="coverage-case">' + options +
            '</select></label><label for="coverage-rank">Rank <select id="coverage-rank">' + rank_options +
            '</select></label></div>'
            f'<h3 id="coverage-title">{html.escape(first["title"])}</h3>'
            f'<iframe id="coverage-chart" src="{first["url"]}" title="{html.escape(first["title"], quote=True)}" loading="lazy"></iframe>'
            f'<p><a id="coverage-csv" href="{first["csv"]}">Download selected timing data (CSV)</a></p>'
            '<details><summary>All focused charts and timing files</summary><ul>' + links + '</ul></details>'
            '<script type="application/json" id="coverage-index">' + data + '</script>'
            """<script>(() => {
              const data = JSON.parse(document.getElementById('coverage-index').textContent);
              const cases = document.getElementById('coverage-case');
              const ranks = document.getElementById('coverage-rank');
              function showChart() {
                const item = data.find(d => d.case === cases.value && d.rank === ranks.value);
                const frame = document.getElementById('coverage-chart');
                frame.src = item.url; frame.title = item.title;
                document.getElementById('coverage-title').textContent = item.title;
                document.getElementById('coverage-csv').href = item.csv;
              }
              cases.addEventListener('change', () => {
                const previous = ranks.value;
                const choices = data.filter(d => d.case === cases.value);
                ranks.replaceChildren(...choices.map(d => new Option(d.rank_label, d.rank)));
                ranks.value = choices.some(d => d.rank === previous) ? previous :
                  choices.some(d => d.rank === 'median') ? 'median' : choices[0].rank;
                showChart();
              });
              ranks.addEventListener('change', showChart);
            })();</script></section>""")


def render(report, output, linear=False):
    if report.get("schema_version") not in (3, 4):
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
    sections = [render_coverage(rows, output, short, linear), render_scaling(rows, output, short, linear)]
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
            colors = [COLORS[r["algorithm"]] for r in selected]
            axis.barh(range(len(selected)), means, xerr=error, height=0.58,
                      color=colors, error_kw={"ecolor": "#172941", "capsize": 3, "linewidth": 1})
            labels = [interactive_charts.label(r["algorithm"]) for r in selected]
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
        figure.text(0.04, 0.015, "Bars: mean · whiskers: 95% confidence interval · colors match the library legends\n"
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
<p>Each library keeps the same color and marker across charts. Rust's built-in selector uses a
dash-dot line with square markers; full-sort baselines use dashed or dotted lines.
Use <strong>Highlight</strong> to follow one library while retaining the others as faint context.
Hover a point to see the library/API name, time and confidence interval. Drag to zoom, use the toolbar to pan or reset,
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
Focused random inputs also rotate over sixteen banks, reduced to four for arrays of at least 64 MiB;
structured inputs use one deterministic bank. Ascending/descending byte cases are sorted after conversion,
so they remain globally ordered above length 256. Periodic cases retain their repeated ramps.
Owned String panels omit tpt-simd-sort because it requires Copy; borrowed str panels include it.
Fixed-case ratios divide mean time by kth's mean in the same panel. Scaling ratios use the selected
baseline at each input size; below 1× means faster than that baseline.
Whiskers describe individual means, not uncertainty in the ratios.
In multiplier views, confidence bands are divided by the baseline mean and do not include baseline uncertainty.</p>
<nav><a href="#coverage">Ranks and difficult inputs</a><a href="#scaling">Performance by input size</a><a href="#partition">Partition</a><a href="#median-only">Median only</a></nav>
SECTIONS
<h2>Run details</h2><p><a href="results.json">Measurements and provenance (JSON)</a></p>
<details><summary>Compiler, machine, dependency versions, and sampling profile</summary><pre>METADATA</pre></details>
</html>"""
    document = document.replace("WARNING_TEXT", html.escape(warning)).replace("SECTIONS", "".join(sections))
    document = document.replace("RUN_LINK", run_link)
    document = document.replace("METADATA", html.escape(json.dumps(metadata, indent=2)))
    document = document.replace('</style>', '.case-controls{display:flex;flex-wrap:wrap;gap:1rem}label{font-weight:600}'
                                'select{display:block;max-width:100%;padding:.6rem;font:inherit;border:1px solid #a9b8cc;'
                                'border-radius:5px;background:white;color:#172941}iframe{width:100%;height:870px;border:0}'
                                '@media(max-width:700px){body{padding:0 .5rem}iframe{height:950px}}'
                                '</style>')
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
    parser.add_argument("--coverage-sizes", type=int, nargs="+",
                        help="Override focused-case lengths (default: curated manifest, including wide-record thresholds and 1M)")
    parser.add_argument("--linear", action="store_true", help="Use linear axes for the size charts instead of log/log")
    args = parser.parse_args()
    sizes = sorted(set(args.sizes))
    coverage_sizes = sorted(set(args.coverage_sizes)) if args.coverage_sizes is not None else None
    if coverage_sizes is not None and any(n <= 0 for n in coverage_sizes):
        parser.error("focused-case lengths must be positive")
    if any(n <= 0 for n in sizes) or len({n | 1 for n in sizes}) < 2:
        parser.error("choose at least two positive sizes with different odd median lengths")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if args.render_only:
        report = json.loads((output / "results.json").read_text())
    else:
        report = measure(output, "quick" if args.quick else "ci" if args.ci else "full", sizes, coverage_sizes)
        # Retain completed measurements even if plotting fails.
        (output / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    render(report, output, args.linear)
    (output / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"Report: {output / 'index.html'}", flush=True)


if __name__ == "__main__":
    main()
