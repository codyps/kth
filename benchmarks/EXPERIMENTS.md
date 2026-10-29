# Selection experiments

These are benchmark-only prototypes against revision `777e864`. The production
selector and public API are unchanged. The small-input optimization in PR #18
is separate; the endgame experiments here also affect narrowed tails of larger
calls, and are not a reproduction of that PR.

Enable the generic candidates with `--features experiments` on the `selection`
benchmark. `control` is the current selector's policy in the common experiment
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
handling for predictable input. No production policy was changed here.

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
iteration count. It supports `kth`, `std`, and selected experiment names, as well
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
