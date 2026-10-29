# Selection experiments

The initial benchmark-only prototypes below compare revision `777e864`.
The byte histogram path was promoted separately as an explicit API in PR #20.
Later adaptive candidates use a post-small-input control, described below.
The initial endgame experiments also affect narrowed tails of larger calls,
and are not a reproduction of the small-input optimization in PR #18.

Enable the generic candidates with `--features experiments` on the `selection`
benchmark. `control` is the frozen `777e864` policy in the common experiment
driver. Compare with both `control` and public `kth`: identical operation counts
can still have different timings because of compiler decisions and code layout.

| Variant | Isolated change |
| --- | --- |
| `small16`, `small32` | Median-of-three three-way selection below 64 elements, finishing with insertion sort at 16 or 32 |
| `network16` | Same small-slice strategy, finishing with a 63-comparator Batcher odd-even merge sorting network, pruned for shorter slices |
| `fr-window` | Floyd–Rivest contiguous sample-window selection above 600 elements; neighboring sample values trigger duplicate handling |
| `fr-sample` | Same window rule, retaining the current nine-sample duplicate trigger instead |
| `expand` | Replace the deterministic fallback and small-slice repeated-step path with central ninthers plus partition expansion |
| `ninther-all` | Run that deterministic selector directly, for comparison with `original` repeated-step selection |
| `block64`, `block128` | Buffered Hoare partitioning with blocks of 64 or 128 offsets; retain the current pivot policy and duplicate handling |
| `ancestor` | Replace repeated-sample duplicate detection with an ancestor-pivot lower bound and an equality-only partition |
| `ancestor-sample` | Combine ancestor-pivot detection with the current sample trigger |
| `sesquick` | Two sampled values, one or two pivots according to relative rank, Yaroslavskiy–Bentley–Bloch partitioning, and a 0.265717 switching boundary |
| `fr-block` | Window sampling except for one-byte values, and block128 partitioning for elements larger than 32 bytes |
| `fr-sample-block` | Window sampling with the original duplicate trigger, and block128 partitioning for elements larger than 32 bytes |

The fixed-sample pivot policy, work budget, min/max scans, and original small
fallback remain in the common driver unless the named experiment replaces them.
Window selection includes its recursive work in timing and operation counts.
The central ninther groups are disjoint: four positions per group from each
outer region and one from the central ninth. Expansion preserves the already
partitioned sample and uses paired exchanges, then two-swap rotations for any
asymmetry. This is a basic implementation, not every optimization in the paper.
The block buffers store `usize` offsets; narrower offset buffers remain a possible
implementation refinement.

The `numeric_experiments` benchmark exposes explicit integer functions rather
than guessing the ordering of an arbitrary `T: Ord` from its representation:

| Variant | Behavior |
| --- | --- |
| `histogram`, `histogram4` | One or four interleaved 256-bin byte histograms, then a three-way partition around the selected value |
| `counting`, `counting4` | The same histogram families followed by sequential reconstruction of the entire sorted byte slice |
| `radix8`, `radix11` | In-place most-significant-digit selection using 8- or 11-bit histograms and three-way digit partitioning |
| `x86-simd` | NumPy's AVX-512 quickselect for u64, pinned to `6e059b14975df2e3c74260ec905167a1be5a34ca` |

All candidates satisfy the complete kth-element partition contract and preserve
the multiset. The byte reconstruction variants fully sort their output, which
also satisfies that contract. Numeric scratch histograms use bounded stack space;
allocation and input cloning are not hidden inside the numeric API. The SIMD
reference is an optional external C++ implementation, not a Rust implementation
added to the crate. It runs single-threaded with AVX2, POPCNT, and AVX-512
F/DQ/BW/VL feature checks.

## Measured results

The local study contains 2,813 timing rows in fourteen cohorts. Selected random
median results are below; these are matched measurements within each cohort.
The generic rows use the faster of public `kth` and `control` as the comparison,
to avoid crediting changes for a slower compiled control path.

| Candidate | Elements | Baseline | Candidate | Elapsed change |
| --- | ---: | ---: | ---: | ---: |
| Byte `counting` | 65,536 | 90.0 µs | 12.7 µs | −85.9% |
| Byte `counting4` | 8,388,608 | 6.68 ms | 1.79 ms | −73.2% |
| 64-byte record `block128` | 65,536 | 402.9 µs | 171.3 µs | −57.5% |
| 64-byte record `fr-sample-block` | 65,536 | 421.0 µs | 147.4 µs | −65.0% |
| 64-byte record `fr-sample-block` | 1,048,576 | 6.35 ms | 3.41 ms | −46.3% |
| Byte `fr-sample` | 65,536 | 88.3 µs | 66.8 µs | −24.3% |
| u64 `fr-sample` | 65,536 | 50.6 µs | 42.0 µs | −17.0% |
| u64 `x86-simd` | 65,536 | 50.5 µs | 30.9 µs | −38.8% |
| u64 `x86-simd` | 8,388,608 | 6.58 ms | 6.87 ms | +4.4% |

These do not support replacing the generic default wholesale. On ordered
large-record inputs block128 can be about 64% slower. Contiguous Floyd–Rivest
sampling has much worse cases: reversed u64 p99 is about 13.4 times slower than
control, and reversed owned-string tail ranks exceed ten times slower. Keeping
the existing duplicate trigger fixes the initial random-byte regression, but
does not remove this sensitivity to input order. A more representative sample
or a policy for ordered input is needed before production adoption.

Other outcomes:

- Central ninthers plus expansion improve direct deterministic selection by
  roughly 5–11% at 65,536 elements, across these five types. The emergency
  fallback is a more promising use than changing every small tail.
- Ancestor-only detection helps random bytes but costs about 50% on all-equal
  or dominant-value cases. Combining both duplicate triggers adds little.
- The plain insertion endgames are mixed; network16 is 24% slower by geometric
  mean in its short sixty-case random-input sweep. PR #18's optimized initial
  small-input handling remains separate.
- The Sesquickselect and radix prototypes lose on the tested random medians.
  Their comparison/scan models and potential improvements should not be
  confused with the elapsed time of these particular implementations.

The strongest next numeric candidate is an explicit byte API using histogram
reconstruction. Block partitioning is a useful large-record kernel, but needs
handling for predictable input. That initial study promoted no generic policy.

## Reproduction

```sh
cargo test --manifest-path benchmarks/Cargo.toml --features experiments --test experiments
KTH_BENCH_SIZES=64,4096,65536 KTH_BENCH_TYPES=u8,u64,String,str,64-byte KTH_BENCH_RANKS=p01,median,p99 KTH_BENCH_INPUTS=16 taskset -c 8 cargo bench --manifest-path benchmarks/Cargo.toml --features experiments --bench selection -- 'selection/.*/random/.*/(kth|control|std|fr-sample|fr-sample-block|block128|ancestor)/' --sample-size 60 --warm-up-time 0.2 --measurement-time 0.8 --nresamples 10000 --noplot
KTH_BENCH_SIZES=65536 KTH_BENCH_TYPES=u8,u64 KTH_BENCH_RANKS=p01,median,p99 KTH_BENCH_INPUTS=16 taskset -c 8 cargo bench --manifest-path benchmarks/Cargo.toml --features experiments --bench numeric_experiments -- 'selection/.*/random/' --sample-size 60 --warm-up-time 0.2 --measurement-time 0.8 --noplot
```

For the SIMD reference, obtain the pinned source and provide its absolute path:

```sh
git clone https://github.com/numpy/x86-simd-sort.git target/x86-simd-sort
git -C target/x86-simd-sort checkout 6e059b14975df2e3c74260ec905167a1be5a34ca
KTH_X86_SIMD_SOURCE="$PWD/target/x86-simd-sort" cargo test --manifest-path benchmarks/Cargo.toml --features native-simd --test experiments
KTH_X86_SIMD_SOURCE="$PWD/target/x86-simd-sort" KTH_BENCH_SIZES=65536 KTH_BENCH_TYPES=u64 KTH_BENCH_RANKS=p01,median,p99 cargo bench --manifest-path benchmarks/Cargo.toml --features native-simd --bench numeric_experiments -- '/random/'
```

This requires g++ and ar. Without the source path, or on an unsupported CPU,
the optional SIMD series is omitted. Default builds do not download or compile
C++ code. `KTH_BENCH_FLAT=1` selects flat Criterion sampling for large arrays;
the regular linear sampling mode can greatly exceed the requested measurement
time when a single selection is expensive.

Operation counts are a separate build, never enabled during timing:

```sh
KTH_BENCH_SIZES=65536 KTH_BENCH_TYPES=u8,u64,String,str,64-byte KTH_BENCH_RANKS=p01,median,p99 KTH_BENCH_INPUTS=16 cargo bench --manifest-path benchmarks/Cargo.toml --features experiments,instrumentation --bench selection_counts > target/experiment-counts.csv
```

The local experiment archive is `target/remaining-experiments/`. Its manifest
separates short screens, longer confirmations, small endgames, deterministic
fallbacks, and large arrays. It retains immutable source snapshots, measured
executables, hashes, raw Criterion samples, and separate count exports. Render
its combined comparison report with:

```sh
uv run --with plotly==6.5.0 python scripts/experiment_report.py target/remaining-experiments
```

The report's interactive heatmap compares all candidates against a matched
baseline and links the full CSV and provenance. Its findings table uses the
faster control where code generation changes otherwise-identical baseline times.
Ordered-input and small-endgame cohorts use shorter samples and are labeled as
screens, rather than mixed into the longer confirmation cohorts.

All local timings use CPU 8 on an AMD Ryzen 9 9955HX with Rust 1.99.0. Screens use
eight seeded random inputs; confirmations use sixteen; large-array runs use two.
Every timed case validates the selected element, both sides, and the full multiset
outside timing. Input cloning is excluded and inputs are warm. Numeric timing uses
matched function-pointer calls for all its candidates, including its baseline;
compare ratios within the same cohort rather than mixing it with the generic
harness. Counts represent logical work, not hardware memory traffic. This is a
shared host; confidence intervals for individual means are not ratio intervals.

## Profiling

The profiling driver resets a preallocated working buffer between calls. Global
`perf stat` counts include that reset and initial input generation; use sampled
profiles to separate reset costs from selection. Profile an optimized build with
debug symbols:

```sh
cargo build --release --manifest-path benchmarks/Cargo.toml --features experiments --example profile_selection
perf stat -r 5 -e cycles,instructions,branches,branch-misses taskset -c 8 benchmarks/target/release/examples/profile_selection kth u8 random 65536 median 20000
perf record -F 999 -e cycles:u --call-graph dwarf,8192 -o target/selection-perf.data -- taskset -c 8 benchmarks/target/release/examples/profile_selection kth u8 random 65536 median 20000
perf report --stdio --no-children -i target/selection-perf.data
```

The driver accepts a selector, element type, distribution, length, rank, and
iteration count. `reset-only` performs the same setup/validation and buffer resets
but skips selection in the repeated loop. It estimates how much of a global
counter run comes from input reset; subtracting separate noisy runs is not a
sampled function profile. It supports `kth`, `std`, and selected experiment names, as well
as the explicit byte and radix paths. One full partition/multiset validation runs
before the loop. Profiling is diagnostic; elapsed-time claims use the separate
Criterion cohorts. Keep profiler builds separate from benchmark builds if adding
frame pointers or other compiler flags.

## Algorithm references

- [Floyd and Rivest, SELECT](https://doi.org/10.1145/360680.360694) and
  [the sample-window equations in quickselect](https://github.com/mourner/quickselect/blob/main/index.js).
  These prototypes combine that sampling rule with our existing partitions;
  they do not reproduce the randomized two-pivot analysis or claim its comparison bound.
- [Alexandrescu, Fast Deterministic Selection](https://erdani.org/research/sea2017.pdf):
  central ninthers and partition expansion. The stale commented outline in
  `src/quickselect.rs` is not used; its outer group offsets would overlap.
- [Edelkamp and Weiß, BlockQuicksort](https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ESA.2016.38):
  buffered classification before movement.
- [Rust's selector](https://github.com/rust-lang/rust/blob/master/library/core/src/slice/sort/select.rs):
  ancestor lower bounds and cheap endgames.
- [Martínez, Nebel, and Wild, Sesquickselect](https://arxiv.org/abs/1810.12322):
  the two-sample switching rule; memory-transfer models are not timing guarantees.
- [NumPy x86-simd-sort](https://github.com/numpy/x86-simd-sort): the actual SIMD
  reference, built from its pinned source through a small C ABI wrapper.

## Adaptive sampling experiments

The `experiments` feature also exposes a frozen copy of the selector after the
small-input improvement (`adaptive-control`) and these variants:

| Variant | Change from that control |
| --- | --- |
| `stratified-mid` | Growing stratified sample for interior ranks at n >= 4096 |
| `adaptive-block` | 128-element buffered partition for large records, gated by disorder |
| `stratified-block` | Both changes above |
| `stratified-all` | Growing samples for tail ranks too, plus gated blocks |
| `guarded` | Raise both thresholds to 16,384, preserve the 17–32-byte sampler, skip growing samples on repeated/predictable input |
| `strict-duplicates` | Require all three sample representatives to match before a three-way pass |
| `bounded-duplicates` | Strict hint plus a discarded ancestor lower bound |
| `guarded-cyclic` | Guarded samples for every width, cyclic partitions for 9–32-byte values |
| `combined` | Guarded/cyclic plus strict hints and ancestor bounds |

The growing sample contains min(2*floor(sqrt(n)), 1024) elements. Deterministic
jitter chooses one element from each disjoint stratum. Forward gathering never
overwrites a future stratum. A rank-dependent inward bias aims to avoid selecting
a sample extreme for tail ranks. The existing selector partitions the sample.
This is a heuristic experiment, not an implementation of SELECT's proof. All
variants retain the four-times-input partition budget and deterministic fallback.
The disorder gate counts changes in the comparison with the pivot at 16 evenly
spaced positions; at least four changes enable the expensive kernel.

An initial short screen covered 160 case/length/rank groups, with n=4096 and
65,536, p01/median/p99, and u8/u64/64-byte/String/str inputs. Equal cases only need
one rank. It used eight seeded banks, 20 flat samples, 20 ms warmup, and 100 ms
measurement, pinned to CPU 8. These are screening results, not final guarantees:

- Ungated growing samples caused substantial small-array and ordered-input
  regressions. Gating and the higher threshold removed most of these losses.
- `guarded` random medians at 65,536 took 0.577x/0.826x/0.381x/0.926x/0.819x
  the control time for u8/u64/64-byte/String/str, respectively. Its worst screen
  regression was 7.4%; four of 160 cases regressed more than 5%.
- Strict duplicate hints cut random-byte time to 0.468x the control, but nearly
  doubled dominant-value median time at n=4096. Ancestor detection did not fix
  that regression. These remain benchmark variants.

Reproduce a screen using the existing selection harness:

```sh
KTH_BENCH_SIZES=4096,65536 KTH_BENCH_TYPES=u8,u64,64-byte,String,str KTH_BENCH_RANKS=p01,median,p99 KTH_BENCH_INPUTS=8 KTH_BENCH_FLAT=1 taskset -c 8 cargo bench --manifest-path benchmarks/Cargo.toml --features experiments --bench selection -- 'selection/.*/(random|sorted|reverse|organ-pipe|sawtooth|equal|dominant|clustered)/.*/(kth|std|adaptive-control|guarded|strict-duplicates|bounded-duplicates)/' --sample-size 20 --warm-up-time 0.02 --measurement-time 0.1 --nresamples 1000 --noplot
```

A broader follow-up screen added one-outlier and string-prefix/variable-length
cases (188 groups, 1,128 timing rows) and compared `guarded`, `guarded-cyclic`,
and `combined`. At n=65,536 the cyclic candidate improved String random median
time by 9.8% and prefix256 by 16.6% versus the control, but sorted String p01
regressed 45.2%. The combined variant retained the dominant-value regression.
Cyclic remains a benchmark variant; swap-based partitioning stays the default.

Local raw samples and frozen binaries are in `target/perf-improvements/`.

### Hardware counter findings

`perf stat` works in this executor. The actual production `kth-u8` API and `std`
were measured with the same optimized profiling binary: eight seeded random
banks, n=65,536, median, 5,000 calls, three repetitions, pinned to CPU 8. Counts
include preallocated buffer reset and initial setup/validation.

| Selector | Cycles | Instructions | Branches | Branch misses |
| --- | ---: | ---: | ---: | ---: |
| `std` | 1,104,079,638 | 6,576,026,494 | 350,549,497 | 151,011 |
| `kth-u8` | 346,005,030 | 1,004,274,535 | 102,533,029 | 51,651 |

The byte API retired 84.7% fewer instructions and used 68.7% fewer cycles in
this diagnostic. Histogram reconstruction reduces work as well as branch misses.
Criterion runs remain the basis for elapsed-time comparisons.

`perf record` could not synthesize process information: this executor exposes
its parent PID namespace through `/proc`. A private namespace with a matching
proc mount was also denied by the operating system. Consequently there is no
sampled call-stack/source-line attribution from this environment; do not interpret
these global counters as a profile of individual functions. The profiling driver
and record/report commands above remain usable on a host with matching mounts.

## Guarded production policy

The public selector uses a conservative refinement of `guarded`: samples can
grow at active lengths >= 16,384 if the fixed sample is distinct and the disorder
probe contains no value equal to the pivot. Elements up to 32 bytes retain the
existing outer-eighth tail pivots; the 17–32-byte recursive sampler is preserved
for every rank. Larger records can grow tail samples too. Disordered records
larger than 32 bytes can use buffered partitioning; other inputs keep their
existing partition kernel. Cyclic remains benchmark-only.

Rejecting equal probes avoids the extra passes observed on few-unique borrowed
strings in the first candidate. Sampling decisions are outlined: placing those
checks in the hot scalar driver caused a reproducible 25% slowdown in ordered
u64 cases in the direct-call harness, despite little difference in the separate
function-pointer profiling driver. Moving the checks out restored those timings.
Recursive sample selection is counted as sampling work in instrumented builds.
The four-times-input budget bounds outer partition work, and sample selection
has its own bounded fallback. The API remains allocation-free and uses safe
swaps throughout these added kernels.
