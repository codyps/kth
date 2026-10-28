# kth

Methods for partitioning arrays & finding the k-th order element.

The median is the element with order equal to half of the array length.

[crates.io](https://crates.io/crates/kth)

[docs (release)](https://docs.rs/kth)

[Rust RFC issue #1470](https://github.com/rust-lang/rfcs/issues/1470) talks about adding a similar interface.

CI checks formatting with rustfmt from Rust 1.99.0. To use the same formatter locally,
install it with `rustup toolchain install 1.99.0 --profile minimal --component rustfmt`
and run `cargo +1.99.0 fmt --all`.
