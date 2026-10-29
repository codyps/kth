"""Join immutable experiment cohorts and render comparisons against matched baselines."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path

import selection_report as selection


def rank_index(rank, n):
    if rank == "min":
        return 0
    if rank == "max":
        return n - 1
    if rank == "next":
        return min(1, n - 1)
    if rank == "penultimate":
        return max(0, n - 2)
    if rank == "median":
        return n // 2
    return n * int(rank[1:]) // 100


def load_cohort(root, cohort, output):
    timings = selection.load_timings(root / cohort["directory"])
    if len(timings) != cohort["expected_rows"]:
        raise ValueError(f'{cohort["name"]}: expected {cohort["expected_rows"]} rows, found {len(timings)}')
    if cohort.get("counts"):
        count_path = output / (cohort["name"] + "-counts.csv")
        with (root / cohort["counts"]).open(newline="") as source, count_path.open("w", newline="") as dest:
            reader = csv.DictReader(source)
            writer = csv.DictWriter(dest, fieldnames=reader.fieldnames)
            writer.writeheader()
            writer.writerows(row for row in reader if row["benchmark"] in timings)
        rows = selection.merge_counts(timings, count_path)
    else:
        rows = list(timings.values())
        for row in rows:
            kind = row["case"].split("/")[0]
            # The retained cohorts use a 64-bit host. These are inline element
            # footprints; string payload allocations are separate.
            width = {"u8": 1, "u64": 8, "String": 24, "str": 16}.get(kind)
            if width is None:
                width = int(kind.removesuffix("-byte"))
            row.update(k=rank_index(row["rank"], row["n"]), element_bytes=width, array_bytes=width * row["n"])
            for metric in ["comparisons_per_element", "scans_per_element", "bytes_moved_per_element", "budget_fallback_rate"]:
                row[metric] = None
    by_id = {row["benchmark"]: row for row in rows}
    baseline = cohort.get("baseline", "kth")
    for row in rows:
        parts = row["benchmark"].split("/")
        parts[-2] = baseline
        base = by_id["/".join(parts)]
        row.update(cohort=cohort["name"], baseline=baseline, baseline_ns=base["mean_ns"],
                   ratio=row["mean_ns"] / base["mean_ns"])
    return rows


def render(cohorts, rows, output):
    import plotly.graph_objects as go
    import interactive_charts

    interactive_charts.write_assets(output)
    figure = go.Figure()
    for index, cohort in enumerate(cohorts):
        selected = [row for row in rows if row["cohort"] == cohort["name"]]
        cases = sorted({(row["case"], row["n"], row["k"], row["rank"]) for row in selected})
        algorithms = sorted({row["algorithm"] for row in selected})
        lookup = {(row["case"], row["n"], row["k"], row["rank"], row["algorithm"]): row for row in selected}
        z, details = [], []
        for algorithm in algorithms:
            values, data = [], []
            for case in cases:
                row = lookup.get((*case, algorithm))
                values.append(math.log2(row["ratio"]) if row else None)
                data.append([row["mean_ns"] / 1000, row["baseline_ns"] / 1000, 100 * (row["ratio"] - 1),
                             row["lower_ns"] / 1000, row["upper_ns"] / 1000, row["ratio"]] if row else [None] * 6)
            z.append(values)
            details.append(data)
        figure.add_trace(go.Heatmap(
            x=[f"{case}<br>{rank}, n={n:,}" for case, n, _, rank in cases], y=algorithms,
            z=z, customdata=details, zmin=-1, zmax=1, colorscale="RdYlGn_r", visible=index == 0,
            colorbar=dict(title="Time / baseline", tickvals=[-1, 0, 1], ticktext=["0.5×", "1×", "2×"]),
            hovertemplate="%{y}<br>%{x}<br>%{customdata[0]:.3f} µs vs %{customdata[1]:.3f} µs"
                          "<br>%{customdata[5]:.3f}× (%{customdata[2]:+.1f}%)"
                          "<br>Mean 95% CI [%{customdata[3]:.3f}, %{customdata[4]:.3f}] µs<extra></extra>",
        ))
    figure.update_layout(
        title=cohorts[0]["name"] + " · baseline: " + cohorts[0].get("baseline", "kth"), height=850, margin=dict(b=180, t=130),
        xaxis=dict(tickfont=dict(size=9), tickangle=-45), yaxis=dict(automargin=True),
        updatemenus=[dict(buttons=[dict(label=cohort["name"], method="update", args=[
            {"visible": [i == index for i in range(len(cohorts))]},
            {"title.text": cohort["name"] + " · baseline: " + cohort.get("baseline", "kth")},
        ]) for index, cohort in enumerate(cohorts)], y=1.12)],
    )
    chart = figure.to_html(full_html=False, include_plotlyjs=False)
    document = ('<!doctype html><meta charset="utf-8"><title>Selection experiments</title>'
                '<script src="plotly.min.js"></script><style>body{max-width:1600px;margin:2rem auto;'
                'padding:1rem;font:16px/1.5 system-ui}pre{white-space:pre-wrap}</style>'
                '<h1>Selection experiments</h1><p><a href="findings.md">Findings</a> · '
                '<a href="comparisons.csv">All comparisons</a> · <a href="results.json">Data and provenance</a></p>'
                '<p>Choose a measurement cohort above the chart. Green means faster; red means slower. '
                'Colors saturate beyond 0.5× and 2×; hover shows exact values. Each ratio uses a baseline '
                'from the same cohort, element type, input distribution, length, and rank. The fallback cohort '
                'uses the original deterministic selector as its baseline. The mean confidence intervals '
                'are individual timing intervals, not intervals for the ratios.</p>' + chart)
    (output / "index.html").write_text(document)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="Directory containing manifest.json and raw Criterion cohorts")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    root = args.input.resolve()
    output = args.output.resolve() if args.output else root / "report"
    output.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((root / "manifest.json").read_text())
    provenances = {}
    for version, spec in manifest["versions"].items():
        provenance = json.loads((root / spec["provenance"]).read_text())
        for relative, digest in provenance["source_sha256"].items():
            if hashlib.sha256((root / spec["snapshot"] / relative).read_bytes()).hexdigest() != digest:
                raise ValueError(f"Snapshot source changed: {version}/{relative}")
        for relative, digest in provenance["binary_sha256"].items():
            if hashlib.sha256((root / spec["binaries"] / relative).read_bytes()).hexdigest() != digest:
                raise ValueError(f"Measured binary changed: {version}/{relative}")
        provenances[version] = provenance
    rows = []
    for cohort in manifest["cohorts"]:
        rows.extend(load_cohort(root, cohort, output))
    report = dict(schema_version=1, manifest=manifest, provenance=provenances, results=rows)
    (output / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    with (output / "comparisons.csv").open("w", newline="") as dest:
        fields = ["cohort", "case", "rank", "n", "algorithm", "baseline", "mean_ns", "baseline_ns", "ratio",
                  "lower_ns", "upper_ns", "comparisons_per_element", "scans_per_element", "bytes_moved_per_element", "budget_fallback_rate"]
        writer = csv.DictWriter(dest, fieldnames=fields, extrasaction="ignore")
        writer.writeheader()
        writer.writerows(rows)
    render(manifest["cohorts"], rows, output)
    print(f"Saved {len(rows)} timing rows from {len(manifest['cohorts'])} cohorts: {output / 'index.html'}")


if __name__ == "__main__":
    main()
