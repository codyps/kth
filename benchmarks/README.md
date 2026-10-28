# Benchmarks

Cross-library comparisons live in the unpublished crate at
[`benchmarks/Cargo.toml`](Cargo.toml). The report's fixed and scaling groups and
the RNG-inclusive groups share one Criterion harness and selector registry. The main crate retains its internal microbenchmarks in
`benches/`; run those with `cargo bench --bench criterion`.

Run commands below from the repository root:

```sh
# Fixed, scaling, and RNG-inclusive comparisons.
cargo bench --manifest-path benchmarks/Cargo.toml --bench comparisons
# Only scaling comparisons, with custom input lengths.
KTH_BENCH_SIZES=16,256,4096,65536 cargo bench --manifest-path benchmarks/Cargo.toml --bench comparisons -- scaling-
# Only fixed median retrieval.
cargo bench --manifest-path benchmarks/Cargo.toml --bench comparisons -- '^median-only/'
```

Every partition workload compares kth, order-stat, pdqselect, adqselect,
tpt-simd-sort, turboselect, and Rust's `select_nth_unstable`, plus stable and
unstable full-sort baselines. Fixed and scaling cases cover seeded random u8,
seeded random u64, and zeros with a leading or trailing one. RNG-inclusive u8
comparisons use the same registry and deliberately include input generation in
their measurements.

Median-retrieval groups compare all nine partition/sort implementations plus
medians on seeded u8 and u64 data. Odd lengths ensure every implementation returns
the same single median. These groups require only the median value: medians' u8
histogram API does not partition its input, so it cannot serve as a partition
competitor. Fixed input cloning and validation are outside timing. Partition
validation checks the selected value, both sides of the partition, and element
preservation; median validation checks the result against a sorted copy.

Turboselect was previously labeled **experimental** because it came from a pinned
Git revision and ran in a separate harness with fewer competitors. All comparisons
now run together under the same sampling and timing protocol. Its Git revision
remains pinned, and the entire comparison crate has `publish = false`, keeping
comparison dependencies out of kth's published manifest.

To run the comparisons and build performance charts, install
[uv](https://docs.astral.sh/uv/) and run:

```sh
uv run scripts/benchmark_report.py
```

Open `target/benchmark-report/index.html`. Hover for exact values, drag to zoom,
pan or reset from the toolbar, click legend entries to hide implementations, or
double-click to isolate one. Confidence bands toggle with their lines.

Each scaling chart has a dropdown to choose **Time (µs)** or **Time / an
implementation**. Relative views divide each time by that implementation's mean
at the **same actual input length**. The dotted 1× line marks the baseline;
0.5× means half its time and 2× means twice its time. Any measured implementation
can be the baseline. A missing baseline point creates a gap rather than an
interpolated comparison. Confidence bands scale each implementation's 95%
confidence interval by the baseline mean; they do not estimate uncertainty in
the ratio. Hover still shows absolute time and its interval.

Both axes use log scales by default; chart buttons switch between log and linear
axes, and `--linear` changes the initial axes. The toolbar exports the current
interactive view as SVG. Standalone PNG/SVG links retain the original absolute-time
views. Fixed-case charts also show time relative to kth in their hover details.

The report records compiler, machine, dependency versions, sampling profile, and
Git revision (including whether the working tree had changes) in `results.json`.
Raw Criterion measurements remain under `raw/`. RNG-inclusive workloads and
internal microbenchmarks are excluded from the report. Every displayed comparison
uses results from the same harness and workload.

The default run uses 100 samples, three seconds of warmup, and five seconds of
measurement per benchmark, sweeping sizes 16, 64, 256, 1,024, 4,096, 16,384, and
65,536. This produces 448 measurements and takes roughly 60 minutes plus setup
and analysis. Use `--sizes 16 256 4096 65536` for a shorter sweep, or add larger
sizes such as 262144 and 1048576. Median inputs use the next odd length when
necessary; the charts show the actual length.

Use `--quick` for a short validation run with visibly marked charts, or `--ci`
for 50 samples, half a second of warmup, and one second of measurement. Use
`--render-only` to redraw existing results without benchmarking, and `--output
PATH` to choose an output directory. Reports made before the unified crate must
be remeasured; timings from separate harnesses are not merged. Python 3.11+ is
required; uv installs plotting dependencies automatically. The report includes
its own `plotly.min.js` and works offline after extracting the full artifact.

The benchmark Nix shell includes Python, matplotlib, and Plotly. With a current
stable Rust toolchain installed (`rustup update stable`), run
`nix develop .#benchmarks --command rustup run stable python3 scripts/benchmark_report.py`.
The comparison dependencies require Rust 1.95 or newer.

See [CONTRIBUTING.md](../CONTRIBUTING.md#benchmark-reports-in-ci) for CI reporting
and publishing.

## Selection optimization benchmarks

Compare kth with its original repeated-step selector and Rust's built-in selector:

```sh
KTH_BENCH_SIZES=16,64,1024,8192,65536 cargo bench --manifest-path benchmarks/Cargo.toml --bench selection
```

The suite checks the minimum, first percentile, median, and maximum on sixteen
seeded random inputs for each length, using both u8 and u64 elements. It also
checks median selection on sorted, reversed, organ-pipe, sawtooth, equal-valued,
and single-outlier inputs. Cloning and correctness checks are outside timing.
For u8 structured inputs, integer values wrap modulo 256; the random u8 and u64
inputs share the same underlying seeded stream. Random median cases also include
owned strings and 64-byte elements to exercise larger, non-`Copy` values and
element-movement costs.
