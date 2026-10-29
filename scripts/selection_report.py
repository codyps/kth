# /// script
# requires-python = ">=3.11"
# dependencies = ["plotly>=6.5,<7"]
# ///
"""Measure focused selection timings and separate operation counts on matched inputs."""

import argparse
import csv
import hashlib
import html
import json
import math
import os
from pathlib import Path
import tempfile

import benchmark_report as common

INTERNAL = ("sampling_comparisons", "partition_comparisons", "fallback_comparisons",
            "elements_scanned", "bytes_moved", "partition_passes", "small_slice_dispatches", "budget_fallbacks")
INTEGER_FIELDS = ("k", "element_bytes", "array_bytes", "calls", "comparisons", *INTERNAL)
COLORS = {"kth": "#2864dc", "swap": "#cc493d", "cyclic": "#d28b12", "rank-before": "#907451", "original": "#8255b5", "std": "#14836c"}
COLORS.update({"entropy8":"#4477aa", "byte-before":"#777777", "compact16":"#ee7733", "lanes8-16":"#cc3311", "merged32":"#009988", "bitmap32":"#aa4499", "before-followup":"#666666", "ordered-hint":"#33bbee"})
COLORS["isolated"] = "#0077bb"
COLORS["kth-u8"] = "#009e73"
COLORS.update({"verified8": "#aa4499", "verified32": "#ddaa33", "verified-duplicates": "#ee7733", "verified-guarded": "#ee3377", "verified-control": "#666666"})
COLORS.update({"combined": "#cc6677", "guarded-cyclic": "#882255", "strict-duplicates": "#88ccee", "bounded-duplicates": "#44aa99", "guarded": "#006699", "adaptive-control": "#555555", "stratified-mid": "#4477aa", "adaptive-block": "#228833", "stratified-block": "#aa3377", "stratified-all": "#ccbb44"})
COLORS.update(dict(zip(
    ("control", "small16", "small32", "fr-window", "expand", "block64", "block128", "ancestor", "ancestor-sample", "sesquick", "histogram", "histogram4", "radix8", "radix11", "x86-simd"),
    ("#666666", "#9c755f", "#bab0ac", "#e15759", "#b07aa1", "#f28e2b", "#edc948", "#59a14f", "#76b7b2", "#af7aa1", "#ff9da7", "#d37295", "#499894", "#86bcb6", "#79706e"),
)))
COLORS.update({"network16": "#117733", "fr-sample": "#228833", "fr-sample-block": "#aa3377", "fr-block": "#e41a1c", "ninther-all": "#984ea3", "counting": "#ff7f00", "counting4": "#a65628"})


def source_hashes():
    paths = [common.ROOT / "Cargo.toml", common.ROOT / "benchmarks/Cargo.toml"]
    paths.extend(p for name in ("Cargo.lock", "benchmarks/Cargo.lock") if (p := common.ROOT / name).exists())
    for directory in ("src", "benchmarks/benches", "benchmarks/tests"):
        paths.extend((common.ROOT / directory).rglob("*.rs"))
    return {str(p.relative_to(common.ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)}


def check_sources(expected):
    actual = source_hashes()
    # Cargo may create lockfiles on the first run. Once present, pin their hashes too.
    created = actual.keys() - expected.keys()
    if any(actual.get(path) != digest for path, digest in expected.items()) or created - {"Cargo.lock", "benchmarks/Cargo.lock"}:
        raise ValueError("Benchmark sources changed during measurement; rerun on a stable checkout before joining timings and counts")
    return actual


def load_timings(raw):
    rows = {}
    for path in sorted(raw.glob("**/new/benchmark.json")):
        name = json.loads(path.read_text())["full_id"]
        suite, kind, case, rank, algorithm, n = name.split("/")
        if suite != "selection" or algorithm not in COLORS or name in rows:
            raise ValueError(f"Unexpected or duplicate benchmark: {name}")
        estimate = json.loads(path.with_name("estimates.json").read_text())["mean"]
        interval = estimate["confidence_interval"]
        mean, low, high = estimate["point_estimate"], interval["lower_bound"], interval["upper_bound"]
        if not all(math.isfinite(x) and x > 0 for x in (mean, low, high)) or not low <= mean <= high:
            raise ValueError(f"Invalid timing: {name}")
        if not math.isclose(interval["confidence_level"], 0.95):
            raise ValueError(f"Expected 95% confidence interval: {name}")
        rows[name] = dict(benchmark=name, case=f"{kind}/{case}", rank=rank, algorithm=algorithm, n=int(n),
                          mean_ns=mean, lower_ns=low, upper_ns=high,
                          sample_count=len(json.loads(path.with_name("sample.json").read_text())["times"]))
    if not rows:
        raise ValueError("No selection timings matched the filter")
    return rows


def merge_counts(timings, path):
    found = {}
    with path.open(newline="") as stream:
        for raw in csv.DictReader(stream):
            name = raw["benchmark"]
            if name not in timings or name in found:
                raise ValueError(f"Unexpected or duplicate count row: {name}")
            row = dict(timings[name])
            for key in INTEGER_FIELDS:
                row[key] = int(raw[key]) if raw[key] else None
                if row[key] is not None and row[key] < 0:
                    raise ValueError(f"Negative {key}: {name}")
            if any(row[key] is None for key in INTEGER_FIELDS[:5]):
                raise ValueError(f"Missing workload dimensions or comparisons: {name}")
            n, calls = row["n"], row["calls"]
            if n <= 0 or calls <= 0 or not 0 <= row["k"] < n or row["array_bytes"] != n * row["element_bytes"]:
                raise ValueError(f"Invalid workload dimensions: {name}")
            opaque = int(raw.get("opaque_sort_calls") or 0)
            opaque_algorithms = {'before-followup', 'ordered-hint', 'adaptive-block', 'adaptive-control', 'bounded-duplicates', 'combined', 'guarded', 'guarded-cyclic', 'isolated', 'kth', 'stratified-all', 'stratified-block', 'stratified-mid', 'strict-duplicates', 'verified-control', 'verified-duplicates', 'verified-guarded', 'verified32', 'verified8'}
            if not 0 <= opaque <= calls or (opaque and row["algorithm"] not in opaque_algorithms):
                raise ValueError(f"Invalid opaque sort count: {name}")
            row["opaque_sort_calls"] = opaque
            if row["algorithm"] == "std":
                if any(row[key] is not None for key in INTERNAL) or raw["budget_fallback_rate"]:
                    raise ValueError("std internal counters must be unavailable")
            else:
                missing = {key for key in INTERNAL if row[key] is None}
                expected = {"elements_scanned", "bytes_moved"} if opaque else set()
                if missing != expected:
                    raise ValueError(f"Missing or unavailable internal counters: {name}")
                if sum(row[key] for key in INTERNAL[:3]) != row["comparisons"]:
                    raise ValueError(f"Comparison phases do not sum to total: {name}")
            for metric, total in (("comparisons_per_element", "comparisons"),
                                  ("scans_per_element", "elements_scanned"),
                                  ("bytes_moved_per_element", "bytes_moved")):
                row[metric] = row[total] / (n * calls) if row[total] is not None else None
            rate = raw["budget_fallback_rate"]
            row["budget_fallback_rate"] = float(rate) if rate else None
            if row["budget_fallback_rate"] is not None and not 0 <= row["budget_fallback_rate"] <= 1:
                raise ValueError(f"Invalid fallback frequency: {name}")
            found[name] = row
    if missing := timings.keys() - found.keys():
        raise ValueError(f"Missing operation counts: {sorted(missing)}")
    return list(found.values())


def measure(output, sampling, settings, pattern):
    sources = source_hashes()
    raw = Path(tempfile.mkdtemp(prefix="run-", dir=output))
    env = dict(os.environ, **settings, CRITERION_HOME=str(raw))
    # A caller's ad hoc counter filter must not silently truncate a matched run.
    env.pop("KTH_BENCH_FILTER", None)
    env.pop("KTH_BENCH_IDS", None)
    command = ["cargo", "bench", "--manifest-path", "benchmarks/Cargo.toml"]
    common.run(command + ["--bench", "selection", "--", pattern] + common.sampling_flags(sampling), env=dict(env))
    sources = check_sources(sources)
    timings = load_timings(raw)
    ids = raw / "measured-ids.txt"
    ids.write_text("\n".join(sorted(timings)) + "\n")
    env["KTH_BENCH_IDS"] = str(ids)
    with (output / "counts.csv").open("w") as stream:
        common.run(command + ["--bench", "selection_counts", "--features", "instrumentation"], env=env, stdout=stream)
    check_sources(sources)
    rows = merge_counts(timings, output / "counts.csv")
    metadata = common.provenance(sampling, raw, sorted({r["n"] for r in rows}))
    metadata["focused_settings"] = {k: v for k, v in env.items() if k.startswith("KTH_BENCH_") and k != "KTH_BENCH_IDS"}
    metadata["criterion_filter"] = pattern
    metadata["source_sha256"] = sources
    metadata["input_count"] = int(env.get("KTH_BENCH_INPUTS", "2" if settings["KTH_BENCH_PROFILE"] == "cache" else "16"))
    metadata.pop("string_and_wide_input_count")
    metadata["input_lengths"] = metadata.pop("scaling_sizes")
    metadata.pop("median_scaling_sizes")
    metadata["linux_cpu0_caches"] = [
        {key: (path / key).read_text().strip() for key in ("level", "type", "size")}
        for path in sorted(Path("/sys/devices/system/cpu/cpu0/cache").glob("index*"))
        if all((path / key).exists() for key in ("level", "type", "size"))
    ]
    return {"schema_version": 1, "metadata": metadata, "results": rows}


def render(report, output):
    import plotly.graph_objects as go
    import interactive_charts

    interactive_charts.write_assets(output)
    rows = report["results"]
    profile = report["metadata"]["focused_settings"]["KTH_BENCH_PROFILE"]
    by_rank = profile == "ranks"
    metrics = {
        "Time (µs)": lambda r: r["mean_ns"] / 1000,
        "Comparisons / element": lambda r: r["comparisons_per_element"],
        "Sampling comparisons / element": lambda r: None if r["sampling_comparisons"] is None else r["sampling_comparisons"] / (r["n"] * r["calls"]),
        "Partition comparisons / element": lambda r: None if r["partition_comparisons"] is None else r["partition_comparisons"] / (r["n"] * r["calls"]),
        "Deterministic comparisons / element": lambda r: None if r["fallback_comparisons"] is None else r["fallback_comparisons"] / (r["n"] * r["calls"]),
        "Scan visits / element": lambda r: r["scans_per_element"],
        "Logical bytes moved / element": lambda r: r["bytes_moved_per_element"],
        "Budget fallback frequency": lambda r: r["budget_fallback_rate"],
    }
    sections = []
    for case in sorted({r["case"] for r in rows}):
        selected = [r for r in rows if r["case"] == case]
        groups = {}
        for row in selected:
            key = row["algorithm"], row["n"] if by_rank else row["rank"]
            groups.setdefault(key, []).append(row)
        series = [sorted(points, key=lambda r: r["k"] if by_rank else r["n"])
                  for _, points in sorted(groups.items())]
        figure = go.Figure()
        for (algorithm, dimension), points in sorted(groups.items()):
            points.sort(key=lambda r: r["k"] if by_rank else r["n"])
            figure.add_trace(go.Scatter(
                x=[r["k"] / max(1, r["n"] - 1) if by_rank else r["array_bytes"] if profile == "cache" else r["n"] for r in points],
                y=[r["mean_ns"] / 1000 for r in points], mode="lines+markers",
                name=f"{algorithm} · {'n=' if by_rank else ''}{dimension}",
                line=dict(color=COLORS[algorithm]),
                error_y=dict(type="data", symmetric=False,
                             array=[(r["upper_ns"] - r["mean_ns"]) / 1000 for r in points],
                             arrayminus=[(r["mean_ns"] - r["lower_ns"]) / 1000 for r in points]),
                customdata=[[r["n"], r["k"], r["array_bytes"], r["mean_ns"] / 1000,
                             r["lower_ns"] / 1000, r["upper_ns"] / 1000] for r in points],
                hovertemplate="%{fullData.name}<br>x=%{x}<br>value=%{y:.6g}<br>n=%{customdata[0]}, k=%{customdata[1]}"
                              "<br>array=%{customdata[2]} bytes<br>time=%{customdata[3]:.6g} µs"
                              "<br>95% CI [%{customdata[4]:.6g}, %{customdata[5]:.6g}] µs<extra></extra>",
            ))
        figure.update_layout(
            title=case, template="plotly_white", height=580,
            xaxis_title="Selected rank k / (n − 1)" if by_rank else "Array footprint (bytes)" if profile == "cache" else "Number of elements",
            xaxis_type="linear" if by_rank else "log", yaxis_title="Time (µs)",
            updatemenus=[dict(buttons=[dict(label=label, method="update", args=[
                {"y": [[value(r) for r in points] for points in series], "error_y.visible": label == "Time (µs)"},
                {"yaxis.title.text": label},
            ]) for label, value in metrics.items()], x=0, y=1.18)],
        )
        sections.append(figure.to_html(full_html=False, include_plotlyjs=False,
                                      config={"toImageButtonOptions": {"format": "svg"}}))
    short = report["metadata"]["profile"] == common.PROFILES["quick"]["label"]
    document = ('<!doctype html><meta charset="utf-8"><title>Selection diagnostics</title>'
                '<script src="plotly.min.js"></script><style>body{max-width:1200px;margin:2rem auto;'
                'padding:1rem;font:16px/1.5 system-ui}pre{white-space:pre-wrap}</style>'
                '<h1>Selection diagnostics</h1>' + ('<p><b>Short validation run; use longer sampling for performance conclusions.</b></p>' if short else '') +
                '<p>Choose timing or a counter metric in each chart. Counts come from a separate untimed run on the same input bank. '
                'Scan visits include revisits and deterministic pivot-group passes; sampled-pivot reads are represented by sampling comparisons. '
                'Logical bytes count value transfers, including temporaries and self-copies, rather than hardware traffic. '
                'String moves count the value header, excluding its heap payload. Rust selector internals and scan/movement counts for opaque tiny sorts are unavailable. '
                'Cloning and correctness validation are excluded. Inputs are freshly cloned, so cache plots describe warm inputs, not a forced cold cache.</p>'
                '<p><a href="results.json">Timings, counts, and provenance</a> · <a href="counts.csv">Raw counts (CSV)</a></p>' +
                ''.join(sections) + '<details><summary>Run details</summary><pre>' +
                html.escape(json.dumps(report["metadata"], indent=2)) + '</pre></details>')
    (output / "index.html").write_text(document)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=common.ROOT / "target/selection-report")
    parser.add_argument("--profile", choices=("ranks", "thresholds", "cache"), default="ranks")
    parser.add_argument("--sizes", nargs="+", type=int)
    parser.add_argument("--cache-bytes", nargs="+", type=int)
    parser.add_argument("--types", nargs="+")
    parser.add_argument("--ranks", nargs="+")
    parser.add_argument("--inputs", type=int)
    parser.add_argument("--filter", default="/random/", help="Criterion regex; default: random inputs")
    sampling = parser.add_mutually_exclusive_group()
    sampling.add_argument("--quick", action="store_true")
    sampling.add_argument("--ci", action="store_true")
    parser.add_argument("--render-only", action="store_true")
    args = parser.parse_args()
    for values in (args.sizes, args.cache_bytes, [args.inputs] if args.inputs is not None else None):
        if values is not None and any(n <= 0 for n in values):
            parser.error("sizes, cache footprints, and input counts must be positive")
    settings = {"KTH_BENCH_PROFILE": args.profile}
    for flag, name in (("sizes", "SIZES"), ("cache_bytes", "CACHE_BYTES"), ("types", "TYPES"), ("ranks", "RANKS")):
        if values := getattr(args, flag):
            settings[f"KTH_BENCH_{name}"] = ",".join(map(str, values))
    if args.inputs is not None:
        settings["KTH_BENCH_INPUTS"] = str(args.inputs)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    path = output / "results.json"
    if args.render_only:
        report = json.loads(path.read_text())
    else:
        report = measure(output, "quick" if args.quick else "ci" if args.ci else "full", settings, args.filter)
        path.write_text(json.dumps(report, indent=2) + "\n")
    render(report, output)
    print(f"Report: {output / 'index.html'}")


if __name__ == "__main__":
    main()
