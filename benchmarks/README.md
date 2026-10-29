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

Partition workloads compare kth, order-stat, pdqselect, adqselect,
tpt-simd-sort, turboselect, and Rust's `select_nth_unstable`, plus stable and
unstable full-sort baselines. Fixed and scaling cases cover seeded random u8,
seeded random u64, and zeros with a leading or trailing one. RNG-inclusive u8
comparisons use the same registry and deliberately include input generation in
their measurements.

The main comparison report also sweeps owned `String`, borrowed `&str`, and
64-byte values. Strings cover random hex data, 64-byte and 256-byte common
prefixes, and seven distinct values. These cases cycle through sixteen seeded
inputs at each length and share the input generator with the focused suite.
Owned strings compare all supported selectors and both sort baselines;
`tpt-simd-sort` is omitted there because it requires `Copy`. Borrowed strings and
64-byte values include it.


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
65,536. This produces 987 measurements and takes roughly 132 minutes plus setup
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

The default rank sweep measures up to 27 distinct positions: minimum, next element,
0.1%, 1%, every fifth percentile, 99%, 99.9%, penultimate, and maximum. Ranks
that map to the same index at small lengths are measured once. Random u8, u64,
owned String, borrowed str, and 64-byte values use the same rank sweep on sixteen
seeded inputs per length. It also
checks median selection on sorted, reversed, organ-pipe, sawtooth, equal-valued,
and single-outlier inputs. Cloning and correctness checks are outside timing.
For u8 structured inputs, integer values wrap modulo 256; the random u8 and u64
inputs share the same underlying seeded stream. Random median cases also include
owned strings and 64-byte elements to exercise larger, non-`Copy` values and
element-movement costs.

The selector variants isolate the optimization experiments:

- `kth`: rank-aware sampled pivots with swap-based partitioning (the default).
- `swap`: frozen median-oriented selector from revision `4f9c8a6`, with its
  original swap partitions, duplicate handling, dispatch thresholds, and fallback.
- `cyclic`: frozen median-oriented pivots with cyclic partitioning; benchmark only.
- `rank-before`: frozen rank-aware selector before the sawtooth three-way
  partition fix, with the same pivot policy as `kth`.

Compare `kth` with `swap` to measure the accumulated rank-aware changes,
`cyclic` with `swap` to measure cyclic partitioning, and `kth` with `rank-before`
to isolate the three-way partition fix. Baselines live under `benches/support/`.
The cyclic ownership/unwind and partition tests live with that benchmark variant
and run through `benchmarks/tests/cyclic.rs`.

Rank-aware selection samples the centers of nine regions for tail ranks. In the
outer eighth it uses the median of three minima/maxima; in the outer 1/84 it uses
the whole sample's minimum/maximum. Center ranks retain the previous pivot rule,
including the larger string samples. The choice is recomputed after each
partition. These are heuristics protected by the unchanged `4 * original_len`
work budget and deterministic fallback. No allocation or randomness was added.

On repeated samples, the three-way partition skips an already-greater suffix
when the active rank is in the lower eighth and elements are at most eight
bytes. This avoids swapping greater values with each other when a biased pivot
is near the minimum. Other passes retain the original three-way loop. The size
restriction also keeps the partition driver small for strings and larger values.

```sh
KTH_BENCH_SIZES=256,4096,65536 cargo bench --manifest-path benchmarks/Cargo.toml --bench selection -- '/(swap|kth)/' --sample-size 50 --warm-up-time 0.2 --measurement-time 1
```

Baselines remain frozen when the current selector changes. Partition kernels and
pivot rules can produce different permutations, so later samples may contain
different values.

The final rank-aware comparison on an AMD Ryzen 9 9955HX with Rust 1.99.0 used
65,536 elements, 16 seeded inputs, and 15 ranks from the second-smallest through
the second-largest element. Each implementation used 60 samples, 0.3-second
warmup, and 1.2-second measurement, pinned to CPU 8. Mean elapsed-time reductions
against `swap` were:

| Element type | First percentile | Median | 99th percentile |
| --- | ---: | ---: | ---: |
| `u8` | 54.4% | 5.3% | 60.2% |
| `u64` | 20.8% | 5.4% | 38.1% |
| `String` | 46.7% | 8.2% | 46.4% |
| `&str` | 19.7% | 4.8% | 27.2% |

A separate structured-input sweep found an 11.4% regression for median selection
on the seven-value `u8` sawtooth (`i % 7`); a 100-sample, two-second repeat
confirmed it. That result predates the three-way partition fix described above.
The pivot policy reduced partition passes but increased time spent exchanging
already-greater values. The local `target/sawtooth-fix/report/` report records
the fix, random-rank checks, neighboring lengths, operation counts, source hashes,
and saved-executable cross-checks.

The fixed selector's 65,536-element sawtooth repeat took 77.9 µs, compared with
90.7 µs for `rank-before` and 85.1 µs for `swap`: 14.1% and 8.5% faster,
respectively. Comparisons and partition passes stayed constant; logical bytes
moved fell by 17.3%. Across five random ranks and five element types, the largest
slowdown versus `rank-before` was 2.1%; byte and u64 timings differed by less than
1%. The final compiled drivers for `String`, `&str`, and 64-byte records have
identical normalized instructions to the saved previous executable. These
measurements use the same host and affinity as the rank-aware experiment above.
One remaining tradeoff is the 4,096-element u64 sawtooth: a short repeat measured
5.61 µs versus 5.48 µs before the fix (+2.4%), still ahead of `swap` at 5.93 µs.

```sh
KTH_BENCH_SIZES=65536 KTH_BENCH_TYPES=u8,u64,String,str,64-byte KTH_BENCH_RANKS=p01,p10,median,p90,p99 KTH_BENCH_INPUTS=16 KTH_BENCH_SWAP_FIRST=1 taskset -c 8 cargo bench --manifest-path benchmarks/Cargo.toml --bench selection -- 'selection/.*/(random|sawtooth|equal)/.*/(swap|rank-before|kth)/' --sample-size 80 --warm-up-time 0.4 --measurement-time 1.6 --noplot
```

These are measurements from one shared host. Set `KTH_BENCH_SWAP_FIRST=1` to
measure the frozen baseline before the current selector; without it the order
is reversed. The final run used baseline-first ordering:

```sh
KTH_BENCH_SIZES=65536 KTH_BENCH_TYPES=u8,u64,String,str KTH_BENCH_RANKS=next,p001,p01,p05,p10,p15,p25,median,p75,p85,p90,p95,p99,p999,penultimate KTH_BENCH_INPUTS=16 KTH_BENCH_SWAP_FIRST=1 taskset -c 8 cargo bench --manifest-path benchmarks/Cargo.toml --bench selection -- 'selection/.*/random/.*/(swap|kth)/' --sample-size 60 --warm-up-time 0.3 --measurement-time 1.2 --noplot
```

The earlier cyclic experiment on the same host compared 111
workload/size combinations using 50 samples per implementation. A confirmation
run repeated 30 random/common-prefix combinations with 100 samples, 0.5-second
warmup, and two-second measurement, pinned to CPU 4. The confirmation reversed
measurement order from swap-first to cyclic-first. For random median selection,
the change in mean elapsed time was:

| Element type | 256 elements | 4,096 elements | 65,536 elements |
| --- | ---: | ---: | ---: |
| `u8` | +14.9% | +21.4% | +40.9% |
| `u64` | +9.1% | +19.8% | +20.0% |
| `String` | -0.7% | -2.0% | +0.7% |
| `&str` | -5.5% | -9.4% | -2.5% |

Negative values mean cyclic was faster. This implementation is an experiment,
not a general performance improvement: scalar regressions outweigh the modest
borrowed-string gains. Small differences and results from one machine should
not be generalized. The initial unpinned sweep showed small u64 gains that did
not survive confirmation. Reproduce the confirmation with:

```sh
KTH_BENCH_SIZES=256,4096,65536 taskset -c 4 cargo bench --manifest-path benchmarks/Cargo.toml --bench selection -- 'selection/(u8|u64|String|str)/(random|prefix256)/(p01|median)/(swap|cyclic)/' --sample-size 100 --warm-up-time 0.5 --measurement-time 2 --noplot
```

The string cases also cover 64-byte and 256-byte common prefixes, variable-length
strings, seven-value inputs, sorted and reversed inputs, and equal values. Each
case measures both owned `String` values and borrowed `&str` values. Random string
cases include first-percentile selection as well as the median.

To isolate string comparisons and save measurements for the current revision:

```sh
KTH_BENCH_SIZES=16,64,1024,8192,65536 cargo bench --manifest-path benchmarks/Cargo.toml --bench selection -- 'selection/(String|str)/[^/]+/(median|p01)/(kth|std)' --save-baseline before-strings
```

After changing the selector, repeat the command with `--baseline before-strings`
in place of `--save-baseline before-strings`. Keep the input sizes, sampling flags,
compiler, and machine the same for both runs.


## Rank, dispatch, and cache diagnostics

Run the focused report to pair ordinary Criterion timings with a separate,
untimed operation-count pass on exactly the measured workload IDs:

```sh
# Dense rank sweep, with quick sampling to validate the report.
uv run scripts/selection_report.py --quick --types u8 u64 --sizes 256 4096 --inputs 4
# Both sides of every length dispatch boundary, including recursive sampling.
uv run scripts/selection_report.py --profile thresholds --types u64 String --ranks median p01 p99
# Large contiguous arrays, using the machine's cache hierarchy on Linux.
uv run scripts/selection_report.py --profile cache --types u64 64-byte --ranks median
```

Open `target/selection-report/index.html`. The dropdown switches between time,
comparisons, sampling comparisons, partition comparisons, deterministic-selector
comparisons, scan visits, logical bytes moved, and budget-fallback frequency.
Rank charts plot the actual index as a fraction of `n - 1`; threshold charts plot
length; cache charts plot array bytes. Timing whiskers show 95% confidence
intervals. Counts are normalized by input elements, and hover retains the actual
length, index, footprint, and timing interval. Results and provenance are in
`results.json`; `counts.csv` contains totals across the `calls` column.
The existing cross-library report remains available separately.

The report accepts `--ci`, `--quick`, `--output PATH`, `--render-only`, and a
Criterion regex via `--filter` (default `/random/`). Use `--filter .` to include
structured inputs. The default sampling uses Criterion's 100 samples, three
seconds of warmup, and five seconds of measurement for every selected case;
these sweeps can take hours. Start with a few types, lengths, and ranks, then
remove `--quick` for measurements used in performance decisions.

The three focused workload presets are also available directly through
`KTH_BENCH_PROFILE` in both `selection` and `selection_counts`:

| Preset | Coverage |
| --- | --- |
| `ranks` (default) | Dense random ranks at the usual lengths; structured median cases and the full string suite. |
| `thresholds` | Lengths 1/2/3, 8/9/10, 63/64/65, 575/576/577, 5,183/5,184/5,185, and 46,655/46,656/46,657. Adds inline values at 7/8/9, 15/16/17, and 31/32/33 bytes around element-size dispatch boundaries. |
| `cache` | Random inline u8, u64, and 64-byte values at footprints from 16 KiB through 64 MiB. On Linux, also measures half, exactly, and twice each data/unified cache size reported for CPU 0, including larger last-level caches. Defaults to two inputs and five ranks (min, p01, median, p99, max). |

Use `KTH_BENCH_TYPES` to limit element types, `KTH_BENCH_RANKS` to limit rank
labels, and `KTH_BENCH_INPUTS` to set the random bank size. These take comma-separated
lists or an integer, respectively. `KTH_BENCH_SIZES` overrides lengths in every
preset. `KTH_BENCH_CACHE_BYTES` overrides cache footprints (decimal bytes), including
on non-Linux systems; the report equivalents are `--types`, `--ranks`, `--inputs`,
`--sizes`, and `--cache-bytes`. String and borrowed-string values belong to the
rank/threshold presets because their payloads are separately allocated.

Cache lengths depend on element size. Large inputs use one iteration per batch
to bound memory consumption. Cloning and validation remain outside timing;
fresh cloning warms the input, so these are working-set sweeps without forced
cache flushing. The report records the detected cache hierarchy and exact array
footprints. String movement counts the String header, not the heap payload.

To export only operation counts:

```sh
KTH_BENCH_SIZES=63,64,65 KTH_BENCH_TYPES=u64,String KTH_BENCH_RANKS=median \
  cargo bench --manifest-path benchmarks/Cargo.toml --bench selection_counts --features instrumentation > counts.csv
```

`KTH_BENCH_FILTER` optionally selects a substring of the full benchmark ID for
this CSV-only command. Counter runs check the selected value, both partition
sides, and element preservation before emitting a row. The report rejects
missing or duplicate IDs when joining counts with timings. It also checks source
hashes before and after both passes, rejecting runs if this shared checkout
changes during measurement.

Counters have explicit algorithmic definitions:

- **Comparisons** count calls to the element's ordering/equality methods, rather
  than individual bytes compared inside strings. A transparent wrapper preserves
  element size and alignment; wrapping can disable library type specializations,
  so these counts describe the instrumented generic path.
- **Sampling / partition / deterministic comparisons** separate sampled-pivot
  work, partition/extreme scans, and the original deterministic selector. The last
  category includes both small-slice dispatches and budget-exhaustion fallbacks.
- **Elements scanned** count partition/extreme-scan visits, including revisits in
  Hoare partitioning and visits to groups in deterministic pivot construction.
  Sampled-pivot reads are represented separately by sampling comparisons.
- **Bytes moved** count logical value transfers: three per swap, including
  self-swaps, and each value copied to/from cyclic temporary storage. These are
  source-level counts, not hardware memory traffic or compiler-emitted loads/stores.
- **Budget fallback frequency** is the fraction of calls exhausting the sampled
  selector's work budget. Small-slice dispatches have their own counter and do
  not inflate this frequency. The original selector has no budget fallback.

Comparisons are available for `original`, `swap`, `cyclic`, `kth`, and `std`.
Internal scan/movement/dispatch counts are available for the first four; unavailable
`std` fields are blank in CSV and null in JSON. Counters are thread-local and
require the opt-in `bench-instrumentation` feature in kth (exposed as
`instrumentation` by the benchmark crate). Ordinary builds remain `no_std`,
and counter hooks compile away with the feature disabled. The timing harness
rejects instrumentation-enabled builds to keep counter overhead out of timings.
