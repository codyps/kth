# Contributing

## Formatting and CI

CI checks formatting with rustfmt from Rust 1.99.0. To use the same formatter locally,
install it with `rustup toolchain install 1.99.0 --profile minimal --component rustfmt`
and run `cargo +1.99.0 fmt --all`.

CI builds and tests with stable, beta, and nightly Rust on Linux, uses the pinned
formatter, checks spelling and changed-line whitespace, and tests the report
generator. Nix is optional for local development; CI does not install Nix or run
duplicate Rust builds through Nix on Linux and macOS.

## Releases

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

## Benchmark reports in CI

CI publishes the latest `main` report at
[codyps.github.io/kth](https://codyps.github.io/kth/). Each push to `main` runs
the **Benchmark report** workflow with pinned Rust 1.99.0, Python 3.12, and
matplotlib 3.10.7 and Plotly 6.5.0. It measures all 1,019 comparisons, builds the interactive
HTML report with selectable relative-time baselines and PNG/SVG exports,
and deploys them to GitHub Pages. The `--ci` profile uses 50 samples, half a second
of warmup, and one second of measurement per comparison (roughly twenty-five minutes
plus setup and analysis). The report links to its Actions run and records its
source revision and environment. Shared runner results can vary between runs.

Pull requests run the short validation profile and upload a downloadable report
artifact without publishing it. Artifacts include raw samples and are retained
for 30 days. To refresh the public site manually, run **Benchmark report** on
`main` using the `ci`, `full`, or `quick` profile. Other branches only produce an
artifact. Pages uses GitHub Actions as its source, with the `github-pages`
environment restricted to deployments from `main`; no extra token is needed.

See the [benchmark guide](benchmarks/README.md) for local runs and report details.
