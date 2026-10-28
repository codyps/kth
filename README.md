# kth

Methods for partitioning arrays & finding the k-th order element.

The k-th order element is the element at index `k` in sorted order. For an
odd-length array, selecting `len / 2` gives the median.

[crates.io](https://crates.io/crates/kth) · [API documentation](https://docs.rs/kth)

Comparison benchmarks against other selection algorithms and full-sort baselines
are available in the [performance report](https://codyps.github.io/kth/).
See the [benchmark guide](benchmarks/README.md) to run them locally.

For development, CI, and release instructions, see [CONTRIBUTING.md](CONTRIBUTING.md).
