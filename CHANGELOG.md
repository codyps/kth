# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1](https://github.com/codyps/kth/compare/v0.1.0...v0.1.1) - 2026-10-28

### Fixed

- fixup readme
- fix nightly
- fix no_std, add some optimization notes
- fix bug in median_of_medians

### Other

- Exclude generated release notes from spellcheck
- Automate release PRs and trusted publishing with release-plz
- Check formatting once with pinned Rust 1.99.0
- Install rustfmt and Clippy for every CI toolchain
- Fix pivot selection, partition tests, and benchmark inputs
- rework tests & benches, add some more helpers
- url
- gitlab ci + nix
- bors & travis are dead
- add more keywords
- reorg to allow usage like other kth libs, add benchmarks comparing using criterion
- travis+bors
- incomplete fr impl
- relicense
- add some top level docs
- hide all internals
- remove some public items
- more readme links
- enable doc gen
- sketch for bfprt_improved, needs expand_partition impl
- include faster partition5
- fix dep versions
- note rust issue
- travis & doc upload
- add a lazy readme
- remove old unused src
- benchmarks, and ones that actually finish (these are _slow_ compared to hash-roll)
- more tests
- benchs & a failing test from a big array (panics)
- no_std, cargo includes
- docs & repo link
- kw & dep fix versions
- test & a panic doc
- slice ext
- names need to be better
- more qc, sort3
- better qc, use TestResult
- m5 improvements & qc
- more things
- partitioning tests ok
