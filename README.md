# kth

Methods for partitioning arrays & finding the k-th order element.

The median is the element with order equal to half of the array length.

[crates.io](https://crates.io/crates/kth)

[docs (release)](https://docs.rs/kth)

[Rust RFC issue #1470](https://github.com/rust-lang/rfcs/issues/1470) talks about adding a similar interface.

CI checks formatting with rustfmt from Rust 1.99.0. To use the same formatter locally,
install it with `rustup toolchain install 1.99.0 --profile minimal --component rustfmt`
and run `cargo +1.99.0 fmt --all`.

Releases use [release-plz](https://release-plz.dev/). Each push to `master`
creates or updates a PR with the next version and changelog. CI runs on that PR;
review and merge it to publish to crates.io and create a GitHub release and tag.
Ordinary merges do not publish, even when `Cargo.toml` has an unpublished version.

Before merging the first release PR, add a GitHub trusted publisher in the
[`kth` crate settings](https://crates.io/crates/kth/settings): owner `codyps`,
repository `kth`, workflow filename `release.yml`, and no environment. The workflow
uses OIDC, so no crates.io token or additional GitHub secret is needed. Repository
Actions settings must allow GitHub Actions to create pull requests.

To retry a failed release or refresh the release PR, run the **Release** workflow
manually on `master`. Formatting remains checked only by the pinned CI formatter.
