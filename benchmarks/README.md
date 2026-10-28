# Benchmarks

Run the commands below from the repository root.

Run comparison benchmarks with `cargo bench --bench criterion`. The partition
benchmarks compare kth, order-stat, pdqselect, Rust's `select_nth_unstable`,
adqselect, and tpt-simd-sort, plus full-sort baselines.
The fixed-input groups clone data outside timing; the `rand` groups include random
input generation in their measurements. New selectors validate their partition
results before timing.

The separate `median-only` groups compare every partition selector and both full-sort
baselines with medians on seeded u8 and u64 data. Each selector partitions at
`len / 2` and returns that element; the sorting baselines return the middle element
after sorting. They use odd lengths so all implementations return the
same single median, and exclude input cloning from timing. These measure only
median retrieval: medians' u8 histogram implementation does not partition input.
Run only those groups with `cargo bench --bench criterion -- median-only`.

The experimental Git-only turboselect comparison lives in a separate, unpublished
harness so the published kth crate requires only registry dependencies. It compares
kth, Rust's selector, and turboselect on the same three fixed datasets, excluding
input cloning from timing and validating results first. Run it with
`cargo bench --manifest-path benchmarks/turboselect/Cargo.toml --bench compare`.
The turboselect revision is pinned in that harness's manifest. Its median-retrieval
suite compares turboselect, kth, Rust's selector, and medians on the same odd-length
u8/u64 datasets, with fixed-size and scaling cases. Run it with
`cargo bench --manifest-path benchmarks/turboselect/Cargo.toml --bench median`.
Its report charts retain their own baselines rather than mixing timings between harnesses.

To run both comparison suites and build performance charts, install
[uv](https://docs.astral.sh/uv/) and run:

```sh
uv run scripts/benchmark_report.py
```

Open `target/benchmark-report/index.html` for charts with mean times, 95% confidence
intervals, and time relative to kth in each fixed dataset. Charts are interactive:
hover for exact values, drag to zoom, pan or reset from the toolbar, click legend
entries to hide implementations, or double-click to isolate one. Confidence bands
toggle with their lines. Each test case also gets
a line graph with one line per implementation: x is the number of elements and y
is mean time. Both axes use log scales by default; shaded bands show 95% confidence
intervals. Full-sort baselines have dashed lines. Switch log/linear axes with the
chart buttons; `--linear` changes the initial axes. The toolbar exports the current
interactive view as SVG.
Standalone PNG/SVG charts and `results.json` are saved alongside it.
The JSON records the compiler, machine,
dependency versions, sampling profile, and Git revision (including whether the
working tree had changes). Raw Criterion measurements are retained under `raw/`.
The report keeps median-only and experimental results separate, and excludes
RNG-inclusive workloads and internal microbenchmarks. Ratios use the baseline
from the same harness and dataset.

The default run uses 100 samples, three seconds of warmup, and five seconds of
measurement per benchmark, sweeping sizes 16, 64, 256, 1,024, 4,096, 16,384, and
65,536. This produces 472 measurements and takes roughly 63 minutes plus setup
and analysis. Use `--sizes 16 256 4096 65536` to choose a shorter sweep, or add
larger sizes such as 262144 and 1048576. Median-only inputs use the next odd length
when necessary, and the line charts show that actual length.
Use `--quick` for a short run to validate the report;
its charts are marked as such. Use `--render-only` to redraw existing results
without benchmarking, or `--output PATH` to choose an output directory. Python
3.11+ is required; uv installs the plotting dependencies automatically. The report
includes its own `plotly.min.js`, so it also works offline after downloading and
extracting the full artifact; it does not load charts from a CDN. PNG/SVG links
retain the original static views for sharing.

The benchmark Nix shell includes Python, matplotlib, and Plotly. With a current stable
Rust toolchain installed (`rustup update stable`), run
`nix develop .#benchmarks --command rustup run stable python3 scripts/benchmark_report.py`.
The comparison dependencies require Rust 1.95 or newer.

Run just the scaling benchmarks with `cargo bench --bench scaling`, or
`cargo bench --manifest-path benchmarks/turboselect/Cargo.toml --bench scaling`.
Both accept a comma-separated `KTH_BENCH_SIZES` environment variable; the report
command sets it from `--sizes` so the harnesses measure matching inputs.


See [CONTRIBUTING.md](../CONTRIBUTING.md#benchmark-reports-in-ci) for CI reporting and publishing.
