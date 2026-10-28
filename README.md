# kth

Methods for partitioning arrays & finding the k-th order element.

The median is the element with order equal to half of the array length.

[crates.io](https://crates.io/crates/kth)

[docs (release)](https://docs.rs/kth)

[Rust RFC issue #1470](https://github.com/rust-lang/rfcs/issues/1470) talks about adding a similar interface.

CI checks formatting with rustfmt from Rust 1.99.0. To use the same formatter locally,
install it with `rustup toolchain install 1.99.0 --profile minimal --component rustfmt`
and run `cargo +1.99.0 fmt --all`.

Releases use [release-plz](https://release-plz.dev/). Each push to `main`
creates or updates a PR with the next version and changelog. CI runs on that PR;
review and merge it to publish to crates.io and create a GitHub release and tag.
Ordinary merges do not publish, even when `Cargo.toml` has an unpublished version.

Before merging the first release PR, add a GitHub trusted publisher in the
[`kth` crate settings](https://crates.io/crates/kth/settings): owner `codyps`,
repository `kth`, workflow filename `release-plz.yml`, and no environment. The workflow
uses OIDC, so no crates.io token is needed.

Release PRs use a GitHub App installed on `kth` with Contents and Pull requests
read/write permissions. Set the Actions variable `RELEASE_PLZ_APP_CLIENT_ID` to its
Client ID and the Actions secret `RELEASE_PLZ_APP_PRIVATE_KEY` to its PEM private key.
The App token lets normal PR checks run automatically without approval prompts.

To retry a failed release or refresh the release PR, run the **Release** workflow
manually on `main`. Formatting remains checked only by the pinned CI formatter.

Run comparison benchmarks with `cargo bench --bench criterion`. The partition
benchmarks compare kth, order-stat, pdqselect, Rust's `select_nth_unstable`,
adqselect, and tpt-simd-sort, plus full-sort baselines.
The fixed-input groups clone data outside timing; the `rand` groups include random
input generation in their measurements. New selectors validate their partition
results before timing.

The separate `median-only` groups compare kth, Rust's selector, and medians on
seeded u8 and u64 data. They use odd lengths so all implementations return the
same single median, and exclude input cloning from timing. These measure only
median retrieval: medians' u8 histogram implementation does not partition input.
Run only those groups with `cargo bench --bench criterion -- median-only`.

The experimental Git-only turboselect comparison lives in a separate, unpublished
harness so the published kth crate requires only registry dependencies. It compares
kth, Rust's selector, and turboselect on the same three fixed datasets, excluding
input cloning from timing and validating results first. Run it with
`cargo bench --manifest-path benchmarks/turboselect/Cargo.toml --bench compare`.
The turboselect revision is pinned in that harness's manifest.

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
65,536. This produces 312 measurements and takes roughly 42 minutes plus setup
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

CI publishes the latest `main` report at
[codyps.github.io/kth](https://codyps.github.io/kth/). Each push to `main` runs
the **Benchmark report** workflow with pinned Rust 1.99.0, Python 3.12, and
matplotlib 3.10.7 and Plotly 6.5.0. It measures all 312 comparisons, builds the interactive
HTML report and PNG/SVG exports,
and deploys them to GitHub Pages. The `--ci` profile uses 50 samples, half a second
of warmup, and one second of measurement per comparison (roughly eight minutes
plus setup and analysis). The report links to its Actions run and records its
source revision and environment. Shared runner results can vary between runs.

Pull requests run the short validation profile and upload a downloadable report
artifact without publishing it. Artifacts include raw samples and are retained
for 30 days. To refresh the public site manually, run **Benchmark report** on
`main` using the `ci`, `full`, or `quick` profile. Other branches only produce an
artifact. Pages uses GitHub Actions as its source, with the `github-pages`
environment restricted to deployments from `main`; no extra token is needed.

CI builds and tests with stable, beta, and nightly Rust on Linux, uses the pinned
formatter, checks spelling and changed-line whitespace, and tests the report
generator. Nix is optional for local development; CI does not install Nix or run
duplicate Rust builds through Nix on Linux and macOS.
