# kth

Methods for partitioning arrays & finding the k-th order element.

The k-th order element is the element at index `k` in sorted order. For an
odd-length array, selecting `len / 2` gives the median.

For numeric byte slices, `kth::partition_u8_by_kth` provides an allocation-free
histogram path for large inputs. It is a separate entry point from the generic
`T: Ord` selector.

[crates.io](https://crates.io/crates/kth) · [API documentation](https://docs.rs/kth)

Comparison benchmarks against other selection algorithms and full-sort baselines
are available in the [performance report](https://codyps.github.io/kth/).
See the [benchmark guide](benchmarks/README.md) to run them locally.
The [selection diagnostics](benchmarks/README.md#rank-dispatch-and-cache-diagnostics)
also measure rank sweeps, dispatch boundaries, cache footprints, and operation counts.

For development, CI, and release instructions, see [CONTRIBUTING.md](CONTRIBUTING.md).
