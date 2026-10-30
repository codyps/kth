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

## Isolated guarded selector

`isolated` exposes the conservative guarded selector through the opt-in
`bench-experiments` library feature. The benchmark crate's `experiments` feature
enables it automatically. Default builds retain the established public selector.

At active lengths >= 16,384, distinct samples and a disorder probe with no
pivot-equal values can enable growing samples. Elements up to 32 bytes retain
the outer-eighth entry pivots, and 17–32-byte values retain recursive sampling.
Predictable or repeated entry samples also take the fixed path. Larger records
can use 128-element buffered partitions. Added kernels use safe swaps, bounded
stack space, and no allocation; the four-times-input outer budget and bounded
sample selection remain. Cyclic stays benchmark-only.

The study found strong random-record gains, but size-dependent regressions
when trying to promote this policy. Moving checks out of a scalar loop fixed
some losses, while other driver refactors changed 16/17-byte and duplicate-heavy
timings substantially. Logical work and elapsed time did not always move together.
The evidence does not support a new generic default. Keeping the experiment
opt-in lets subsequent work investigate code generation and cache behavior
without changing the production call graph.

The fixed selector retains its original body and annotations. Feature-only
wrappers share its safe kernels with the experiment. Instrumented recursive
sample selection is counted as sampling work. `reset-only` profiling runs help
separate buffer-reset costs from selection in global hardware counters; sampled
source-line profiles remain unavailable in this executor.

## Verifying weak duplicate hints

Requiring all three representatives to match lost the dominant-value fast path.
The next experiments instead check a weak repeated-sample hint against jittered
probes. Stop at the first value equal to the pivot; if none match, use a two-way
pass. This verification is limited to active lengths >= 1024 and interior ranks
outside the outer eighths. Every variant retains the existing work budget.

| Variant | Policy |
| --- | --- |
| `verified-duplicates` | 16 probes, otherwise the fixed post-small-input policy; compare with `adaptive-control` |
| `verified-control` | Conservative per-pass growing-sample/block policy in the shared experimental driver, without hint verification |
| `verified-guarded` | The experimental control plus 16-probe verification |
| `verified8`, `verified32` | The same verification with 8 or 32 strata |

Compare the combined variants with `verified-control`; the public `kth` series
shows the unchanged default; `isolated` exposes the guarded entry-dispatch experiment. These are benchmark-only variants,
not production dispatch changes.

A short 1,128-row screen of the first three variants covered 188 groups, with
eight banks and the same 20-sample settings as above. A separate quiet 72-row
confirmation compared the three probe counts, their control, `kth`, and `std`
on twelve focused groups. It used sixteen seeded banks, 60 flat samples, 150 ms
warmup, 600 ms measurement, and 10,000 resamples, pinned to CPU 8. No local builds
or other measurements overlapped this confirmation.

| Case | n | Control | 16 probes | Change |
| --- | ---: | ---: | ---: | ---: |
| u8 random median | 65,536 | 64.44 µs | 47.25 µs | −26.7% |
| u8 sawtooth median | 65,536 | 77.35 µs | 65.47 µs | −15.4% |
| u8 sawtooth median | 4,096 | 6.57 µs | 6.56 µs | −0.2% |
| u8 dominant median | 4,096 | 1.660 µs | 1.665 µs | +0.3% |
| 64-byte sawtooth median | 65,536 | 272.44 µs | 174.37 µs | −36.0% |

The sixteen-probe candidate's worst confirmed loss was 3.6% on u64 clustered
p01, compared with 8.2% and 7.3% for eight and thirty-two probes. That tail rank
does not enable verification, so this difference also illustrates code-generation
and timing sensitivity. The large structured sawtooth gains did not carry over
to the other probe counts. More probes are not uniformly better. Sixteen probes
beat `std` by 18.0% on the large random-byte median in this cohort, still short
of a 20% margin. The explicit histogram API remains the stronger byte path.

Reproduce the focused probe-count comparison with:

```sh
KTH_BENCH_SIZES=4096,65536 KTH_BENCH_TYPES=u8,u64,64-byte KTH_BENCH_RANKS=p01,median KTH_BENCH_INPUTS=16 KTH_BENCH_FLAT=1 taskset -c 8 cargo bench --manifest-path benchmarks/Cargo.toml --features experiments --bench selection -- 'selection/(u8/(random|sawtooth|dominant|clustered)/median|u64/clustered/p01|64-byte/sawtooth/median)/(kth|std|verified-control|verified-guarded|verified8|verified32)/' --sample-size 60 --warm-up-time 0.15 --measurement-time 0.6 --nresamples 10000 --noplot
```

The archive retains raw samples, commands, source snapshots, and binaries in
`target/perf-improvements/`. Earlier overlapping-build cohorts are diagnostic
only; use the separately labeled quiet confirmations for promotion evidence.

## Histogram lanes and ordered pivot hints

The next byte experiments isolate counter width, update dependencies and bin
reconstruction. `byte-before` freezes the explicit byte implementation at
`fbb3235`; it delegates small and extreme-rank calls to the public generic API.

| Variant | Change from `byte-before` |
| --- | --- |
| `compact16` | Four u16 histograms for lengths <= 65,535 |
| `lanes8-16` | Eight u16 histograms at the same size limit |
| `merged32` | Merge four u32 histograms before reconstructing the slice |
| `bitmap32` | Also use occupancy bitmaps to skip empty bins |
| `entropy8` | Choose eight narrow lanes only when three separated windows suggest concentrated data |

The blanket eight-lane variant helps dominant values but loses about 19% on
1,024 sorted bytes in the initial screen. Merged counts and occupancy bitmaps
also have material losses. Only concentration-based dispatch is promoted to
`partition_u8_by_kth`: at least 36 of 48 probes from the first, middle and last
windows must equal the middle byte. This is only a kernel hint; both kernels
still count every byte. The size bound prevents narrow-counter overflow, and
eight u16 lanes retain the original four KiB of histogram storage. Small slices,
min/max ranks and lengths above 65,535 retain the previous paths.

The final byte-only confirmation used sixteen input banks, CPU 8, 40 flat
samples, 50 ms warmup and 300 ms measurement at five lengths. Selected
dominant-byte medians:

| Elements | Frozen baseline | Concentration dispatch | Time reduction |
| ---: | ---: | ---: | ---: |
| 1,024 | 499 ns | 452 ns | 9.5% |
| 4,096 | 1,474 ns | 1,020 ns | 30.8% |
| 16,384 | 5,136 ns | 3,173 ns | 38.2% |
| 65,535 | 19,924 ns | 11,888 ns | 40.3% |

`perf stat -r 3` on 40,000 dominant-byte median calls at length 16,384 recorded
996.2 million cycles for `byte-before` and 614.2 million for the promoted kernel
(38.3% fewer). Retired instructions fell from 2,112.1 to 1,950.5 million (7.7%).
The profile includes buffer resets and setup; the reduction is consistent with
shortening dependent histogram updates, rather than just removing instructions.

The generic `before-followup` variant freezes the full public policy at
`fbb3235`. `ordered-hint` adds an ascending-input hint only for eight-byte
values, active lengths >= 16,384 and ranks outside the outer eighths. Sixteen
jittered strata must be strictly ascending, as must first/middle/last values;
repeated representatives disable the hint. It chooses the requested index as
pivot but still executes a complete partition under the original work budget.
Misleading probes therefore cannot bypass correctness or the fallback.

The ordered candidate approximately halves sorted-u64 median time at 16K, 64K
and 1M elements. A 64K profile also halves retired instructions. It remains a
benchmark variant: short screens showed large unrelated timing losses, and
longer confirmations and separately compiled controls differed materially.
These runs do not establish a sufficiently stable generic default improvement.
Swap partitioning remains the default, and cyclic partitioning stays experimental.

Raw samples, commands, immutable measured binaries and source snapshots are in
`target/target20-followup/`; its report distinguishes the rejected combined
production trial from the final byte-only implementation. Builds and tests did
not overlap the retained timing runs. All 105 byte groups in the final cohort clear a 20% margin against `std`, but
the earlier longer byte cohort misses two 1,024-byte groups. The largest final
loss against the frozen baseline is 11% on 1,024 clustered bytes, where sampling
adds overhead. Results vary across runs; this is not evidence of a universal
20% margin. Several generic workloads also remain below the target.

Coverage limitation: the existing numeric byte cases named `sorted` and
`reverse` narrow integer ramps to u8, wrapping every 256 positions. They are
periodic inputs, not globally ordered byte slices above length 256. Historical
measurements retain those exact inputs and labels. Truly ordered byte inputs
need separate coverage before drawing conclusions about that distribution.

## Dispersed bytes and the full narrow-counter range

The follow-up keeps the concentration hint and adds a conservative diversity
hint: nine prefix bytes must occupy at least eight different buckets in their
low five bits. Dispersed data uses one u32 histogram up to 2,048 elements and
two above that. This reduces counter initialization and bin summation for short
inputs. The seven-value sawtooth fails the diversity hint and keeps four lanes.
Misleading hints only change the kernel; every byte is still counted.

Eight narrow lanes now cover lengths through 524,280: each receives at most
ceil(n / 8) bytes, including one remainder byte, so every u16 counter fits.
Larger slices retain the four-lane implementation. Matching neighbors within
the middle sample window choose the anchor, so an isolated middle outlier does
not hide a dominant value. Histograms still use at most four KiB of scratch.

New `ordered-asc` and `ordered-desc` cases sort bytes after narrowing, across
focused timing, operation-count and profiler inputs. Their monotonicity is
checked in a regression test. Historical `sorted` and `reverse` cases keep
their original wrapped ramps, preserving comparisons with earlier reports.

| Benchmark variant | Purpose |
| --- | --- |
| `lanes-before` | Freeze the PR #24 explicit byte selector at `52e6222` |
| `lanes1-32`, `lanes2-32` | Unconditionally use one or two u32 histograms |
| `ordered-bytes` | Verify a monotonicity hint, then return or reverse |
| `adaptive-lanes` | Combine concentration and a broader diversity bitmap in one sample loop |
| `sampled-lanes` | Separate the cheap concentration and nine-byte diversity checks, as in the promoted implementation |

One or two lanes alone lose badly on dominant and sawtooth distributions.
The broad combined classifier adds up to 14% on small sawtooth inputs. The
ordered-input experiment helps at length 1,024, but can nearly double time on
clustered data while checking a long ordered prefix. These remain benchmark
variants; only the separate lane hints are promoted.

The final confirmation contains 405 rows, or 135 matched groups. It uses
sixteen banks, CPU 8, 40 flat samples, 50 ms warmup, 300 ms measurement and
5,000 resamples. Selected medians against the frozen PR #24 implementation:

| Case | Elements | Before | After | Time reduction |
| --- | ---: | ---: | ---: | ---: |
| Random | 1,024 | 638 ns | 570 ns | 10.7% |
| Random | 4,096 | 1,165 ns | 1,075 ns | 7.7% |
| Random | 65,536 | 13.24 µs | 12.21 µs | 7.8% |
| Random | 524,280 | 104.13 µs | 99.02 µs | 4.9% |
| Dominant | 65,536 | 20.01 µs | 11.86 µs | 40.7% |
| Dominant | 524,280 | 157.66 µs | 92.19 µs | 41.5% |

The largest final baseline loss was 6.7% on wrapped-reverse p99 at length 4,096;
small sawtooth cases also add a few percent. The implementation clears the 20%
margin against `std` in 133 of 135 groups. The remaining two are random p01
(589.7 vs 729.9 ns, 19.2% lower) and wrapped-ramp p01 (488.8 vs 562.1 ns, 13.1%
lower), both at length 1,024. An earlier cohort cleared all 135 groups;
run-to-run timing differences still matter. Generic workloads remain outside
this byte-specific improvement, and the every-case target is still unmet.

Separate `perf stat -r 3` runs include setup, validation and buffer resets:

| Case | Calls | Cycles before / after | Instructions before / after |
| --- | ---: | ---: | ---: |
| Random, n=1,024 | 300,000 | 922.4M / 839.2M | 3,543.0M / 2,970.7M |
| Random, n=65,536 | 20,000 | 1,331.5M / 1,231.2M | 3,912.0M / 3,894.9M |
| Dominant, n=65,536 | 20,000 | 1,978.7M / 1,203.9M | 3,839.5M / 3,388.8M |
| Dominant, n=524,280 | 5,000 | 3,903.3M / 2,351.6M | 7,489.6M / 6,513.5M |

These support less summation/initialization work on small dispersed bytes and
shorter dependent counter-update chains on dominant values. Global counters
are not sampled source-line attribution. The archive at `target/target20-followup/`
retains the final commands, source snapshots, binaries, samples and counters.

## Input rotation across Criterion callbacks

The timing harness keeps the input-bank cursor outside Criterion's callback.
Criterion invokes that callback separately for its iteration counts, including
individual samples. A cursor initialized inside the callback can keep selecting
the first bank when there is only one iteration per sample, or overweight the
first few banks when the iteration count is small. The cursor now continues
across those calls in the selection, numeric and cross-library harnesses.
Cloning and validation remain outside the measured selector.

Earlier snapshots preserve their original behavior and should not be described
as balanced bank measurements without checking their iteration counts. The
standalone perf driver already rotates over its entire repeated loop. Structured
distributions generally use one deterministic input; configured bank counts
refer to the seeded random-input banks.
