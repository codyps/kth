//! Identical workload enumeration for timing and operation-count runs.
use crate::{inputs, sizes};
use std::{collections::BTreeSet, fmt::Debug, mem::size_of};

pub trait Runner {
    fn run<T: Ord + Clone + Debug>(&mut self, name: &str, inputs: &[Vec<T>], rank: &str, k: usize);
}

const THRESHOLD_SIZES: &[usize] = &[
    1, 2, 3, 8, 9, 10, 63, 64, 65, 575, 576, 577, 5183, 5184, 5185, 46655, 46656, 46657,
];
// Portable footprints, augmented with the host's data/unified cache boundaries on Linux.
const CACHE_BYTES: &[usize] = &[
    16 << 10,
    32 << 10,
    64 << 10,
    128 << 10,
    256 << 10,
    512 << 10,
    1 << 20,
    2 << 20,
    4 << 20,
    8 << 20,
    16 << 20,
    32 << 20,
    64 << 20,
];

fn cache_bytes() -> Vec<usize> {
    if let Ok(value) = std::env::var("KTH_BENCH_CACHE_BYTES") {
        let values: BTreeSet<usize> = value
            .split(',')
            .map(|s| {
                s.trim()
                    .parse()
                    .expect("KTH_BENCH_CACHE_BYTES must contain integers")
            })
            .collect();
        assert!(
            values.iter().all(|n| *n > 0),
            "cache footprints must be positive"
        );
        return values.into_iter().collect();
    }
    let mut bytes: BTreeSet<_> = CACHE_BYTES.iter().copied().collect();
    if let Ok(entries) = std::fs::read_dir("/sys/devices/system/cpu/cpu0/cache") {
        for entry in entries.flatten() {
            let path = entry.path();
            let kind = std::fs::read_to_string(path.join("type")).unwrap_or_default();
            if !matches!(kind.trim(), "Data" | "Unified") {
                continue;
            }
            let size = std::fs::read_to_string(path.join("size")).unwrap_or_default();
            let size = size.trim();
            let (digits, unit) = size.strip_suffix('K').map_or_else(
                || size.strip_suffix('M').map_or((size, 1), |v| (v, 1 << 20)),
                |v| (v, 1 << 10),
            );
            if let Some(n) = digits
                .parse::<usize>()
                .ok()
                .and_then(|n| n.checked_mul(unit))
                .filter(|n| *n > 1)
            {
                bytes.extend([n / 2, n]);
                if let Some(larger) = n.checked_mul(2) {
                    bytes.insert(larger);
                }
            }
        }
    }
    bytes.into_iter().collect()
}

pub struct Config {
    pub profile: String,
    pub input_count: usize,
    lengths: Option<Vec<usize>>,
    ranks: Option<BTreeSet<String>>,
    types: Option<BTreeSet<String>>,
}

fn list(name: &str) -> Option<BTreeSet<String>> {
    std::env::var(name).ok().map(|s| {
        let values: BTreeSet<_> = s.split(',').map(|v| v.trim().to_owned()).collect();
        assert!(
            !values.contains(""),
            "{name} must be a nonempty comma-separated list"
        );
        values
    })
}

impl Config {
    pub fn from_env() -> Self {
        let profile = std::env::var("KTH_BENCH_PROFILE").unwrap_or_else(|_| "ranks".into());
        assert!(
            matches!(profile.as_str(), "ranks" | "thresholds" | "cache"),
            "unknown KTH_BENCH_PROFILE"
        );
        let input_count = std::env::var("KTH_BENCH_INPUTS").map_or_else(
            |_| if profile == "cache" { 2 } else { 16 },
            |s| s.parse().expect("KTH_BENCH_INPUTS must be an integer"),
        );
        assert!(input_count > 0, "KTH_BENCH_INPUTS must be positive");
        let ranks = list("KTH_BENCH_RANKS");
        if let Some(names) = &ranks {
            for name in names {
                assert!(
                    rank_candidates(10_000).iter().any(|(r, _)| r == name),
                    "unknown rank: {name}"
                );
            }
        }
        let types = list("KTH_BENCH_TYPES");
        if profile == "cache" {
            assert!(
                !types
                    .as_ref()
                    .is_some_and(|t| t.contains("String") || t.contains("str")),
                "cache profile requires inline values; use ranks for strings"
            );
        }
        if let Some(names) = &types {
            for name in names {
                assert!(
                    [
                        "u8", "u64", "String", "str", "7-byte", "9-byte", "15-byte", "16-byte",
                        "17-byte", "31-byte", "32-byte", "33-byte", "64-byte"
                    ]
                    .contains(&name.as_str()),
                    "unknown element type: {name}"
                );
            }
        }
        Self {
            profile,
            input_count,
            lengths: std::env::var_os("KTH_BENCH_SIZES").map(|_| sizes::sizes()),
            ranks,
            types,
        }
    }

    fn sizes<T>(&self) -> Vec<usize> {
        if let Some(lengths) = &self.lengths {
            return lengths.clone();
        }
        match self.profile.as_str() {
            "thresholds" => THRESHOLD_SIZES.to_vec(),
            "cache" => cache_bytes()
                .iter()
                .map(|bytes| bytes.div_ceil(size_of::<T>()))
                .collect(),
            _ => sizes::sizes(),
        }
    }

    fn includes(&self, name: &str, threshold_only: bool) -> bool {
        self.types.as_ref().map_or_else(
            || !threshold_only || self.profile == "thresholds",
            |types| types.contains(name),
        )
    }

    fn ranks(&self, n: usize) -> Vec<(String, usize)> {
        let mut seen = BTreeSet::new();
        let mut ranks: Vec<_> = rank_candidates(n)
            .into_iter()
            .filter(|(name, k)| {
                let selected = self.ranks.as_ref().map_or_else(
                    || {
                        self.profile != "cache"
                            || ["min", "p01", "median", "p99", "max"].contains(&name.as_str())
                    },
                    |ranks| ranks.contains(name),
                );
                selected && seen.insert(*k)
            })
            .collect();
        ranks.sort_by_key(|(_, k)| *k);
        ranks
    }

    fn random<T: Ord + Clone + Debug>(
        &self,
        runner: &mut impl Runner,
        name: &str,
        bank: &[Vec<T>],
    ) {
        for (rank, k) in self.ranks(bank[0].len()) {
            runner.run(&format!("{name}/random"), bank, &rank, k);
        }
    }
}

fn rank_candidates(n: usize) -> Vec<(String, usize)> {
    assert!(n > 0);
    // Preserve familiar labels when several quantiles collapse to one index.
    let mut ranks = vec![
        ("min".into(), 0),
        ("max".into(), n - 1),
        ("p01".into(), n / 100),
        ("median".into(), n / 2),
        ("next".into(), 1.min(n - 1)),
        ("penultimate".into(), n.saturating_sub(2)),
        ("p001".into(), n / 1000),
        ("p999".into(), n / 1000 * 999 + n % 1000 * 999 / 1000),
    ];
    for percentile in (5..100).step_by(5).chain([99]) {
        if percentile != 50 {
            ranks.push((
                format!("p{percentile:02}"),
                n / 100 * percentile + n % 100 * percentile / 100,
            ));
        }
    }
    ranks
}

fn numeric<T: Ord + Clone + Debug>(
    config: &Config,
    runner: &mut impl Runner,
    name: &str,
    convert: impl Fn(u64) -> T,
) {
    for n in config.sizes::<T>() {
        let bank = inputs::random_bank_of(n, config.input_count, &convert);
        config.random(runner, name, &bank);
        if config.profile == "cache" {
            continue;
        }
        // Release the random bank before materializing the structured inputs.
        drop(bank);
        let mut cases = vec![
            "sorted",
            "reverse",
            "organ-pipe",
            "sawtooth",
            "equal",
            "leading-one",
            "trailing-one",
        ];
        if cfg!(feature = "experiments") {
            cases.extend(["dominant", "clustered"]);
        }
        if name == "u8" {
            cases.extend(["ordered-asc", "ordered-desc"]);
        }
        for case in cases {
            let mut values: Vec<_> = (0..n)
                .map(|i| {
                    convert(match case {
                        "sorted" | "ordered-asc" | "ordered-desc" => i as u64,
                        "reverse" => (n - 1 - i) as u64,
                        "organ-pipe" => i.min(n - 1 - i) as u64,
                        "sawtooth" => (i % 7) as u64,
                        "equal" => 0,
                        "leading-one" => u64::from(i == 0),
                        "trailing-one" => u64::from(i == n - 1),
                        "dominant" if i % 10 != 0 => 5,
                        "clustered" if i < n * 9 / 10 => 5,
                        "dominant" | "clustered" => (i as u64).wrapping_mul(6364136223846793005),
                        _ => unreachable!(),
                    })
                })
                .collect();
            if case.starts_with("ordered-") {
                values.sort_unstable();
                if case == "ordered-desc" {
                    values.reverse();
                }
            }
            // Structured workloads retain their focused median coverage.
            if case.starts_with("ordered-")
                || case == "dominant"
                || case == "clustered"
                || (cfg!(feature = "experiments")
                    && matches!(case, "sorted" | "reverse" | "organ-pipe"))
            {
                for (rank, k) in config.ranks(n) {
                    runner.run(
                        &format!("{name}/{case}"),
                        std::slice::from_ref(&values),
                        &rank,
                        k,
                    );
                }
            } else if config.ranks.as_ref().is_none_or(|r| r.contains("median")) {
                runner.run(&format!("{name}/{case}"), &[values], "median", n / 2);
            }
        }
    }
}

fn bytes<const N: usize>(x: u64) -> [u8; N] {
    // Keep low bits for values shorter than u64 so structured outliers survive.
    std::array::from_fn(|i| x.to_be_bytes()[(i + 8usize.saturating_sub(N)) % 8])
}

pub fn visit(config: &Config, runner: &mut impl Runner) {
    if config.includes("7-byte", true) {
        numeric(config, runner, "7-byte", bytes::<7>);
    }
    if config.includes("9-byte", true) {
        numeric(config, runner, "9-byte", bytes::<9>);
    }
    if config.includes("15-byte", true) {
        numeric(config, runner, "15-byte", bytes::<15>);
    }
    if config.includes("31-byte", true) {
        numeric(config, runner, "31-byte", bytes::<31>);
    }
    if config.includes("u8", false) {
        numeric(config, runner, "u8", |x| x as u8);
    }
    if config.includes("u64", false) {
        numeric(config, runner, "u64", |x| x);
    }
    if config.includes("64-byte", false) {
        numeric(config, runner, "64-byte", |x| [x; 8]);
    }
    if config.includes("16-byte", true) {
        numeric(config, runner, "16-byte", bytes::<16>);
    }
    if config.includes("17-byte", true) {
        numeric(config, runner, "17-byte", bytes::<17>);
    }
    if config.includes("32-byte", true) {
        numeric(config, runner, "32-byte", bytes::<32>);
    }
    if config.includes("33-byte", true) {
        numeric(config, runner, "33-byte", bytes::<33>);
    }
    // The cache profile measures contiguous arrays; strings add separately allocated payloads.
    if config.profile == "cache" {
        return;
    }
    for n in config.sizes::<String>() {
        if !config.includes("String", false) && !config.includes("str", false) {
            break;
        }
        let random = inputs::random_bank_of(n, config.input_count, |x| x);
        for case in inputs::string_cases(true) {
            let words = inputs::strings(case, &random);
            let ranks = if case == "random"
                || (cfg!(feature = "experiments") && matches!(case, "sorted" | "reverse"))
            {
                config.ranks(n)
            } else {
                if config.ranks.as_ref().is_some_and(|r| !r.contains("median")) {
                    continue;
                }
                vec![("median".into(), n / 2)]
            };
            if config.includes("String", false) {
                for (rank, k) in &ranks {
                    runner.run(&format!("String/{case}"), &words, rank, *k);
                }
            }
            if config.includes("str", false) {
                let borrowed: Vec<Vec<&str>> = words
                    .iter()
                    .map(|v| v.iter().map(String::as_str).collect())
                    .collect();
                for (rank, k) in &ranks {
                    runner.run(&format!("str/{case}"), &borrowed, rank, *k);
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;

    fn config(profile: &str) -> Config {
        Config {
            profile: profile.into(),
            input_count: 2,
            lengths: None,
            ranks: None,
            types: None,
        }
    }

    #[test]
    fn short_values_preserve_structured_integer_order() {
        for x in 0..1024 {
            assert!(bytes::<7>(x) < bytes::<7>(x + 1));
        }
    }

    #[test]
    fn ordered_byte_workloads_are_monotone_after_narrowing() {
        struct Check(usize);
        impl Runner for Check {
            fn run<T: Ord + Clone + Debug>(
                &mut self,
                name: &str,
                bank: &[Vec<T>],
                _: &str,
                _: usize,
            ) {
                if name == "u8/ordered-asc" || name == "u8/ordered-desc" {
                    self.0 += 1;
                    for values in bank {
                        assert!(values.windows(2).all(|pair| if name.ends_with("asc") {
                            pair[0] <= pair[1]
                        } else {
                            pair[0] >= pair[1]
                        }));
                    }
                }
            }
        }
        let mut settings = config("ranks");
        settings.lengths = Some(vec![1024]);
        settings.ranks = Some(["median".into()].into());
        let mut check = Check(0);
        numeric(&settings, &mut check, "u8", |v| v as u8);
        assert_eq!(check.0, 2);

        for reverse in [false, true] {
            let values = inputs::ordered_bytes(1024, reverse);
            assert!(values.windows(2).all(|pair| if reverse {
                pair[0] >= pair[1]
            } else {
                pair[0] <= pair[1]
            }));
            let mut counts = [0; 256];
            for v in values {
                counts[v as usize] += 1;
            }
            assert_eq!(counts, [4; 256]);
        }
    }

    #[test]
    fn ranks_are_valid_unique_and_dense() {
        let config = config("ranks");
        for n in [1, 2, 3, 8, 16, 64, 65, 1000, 65536, usize::MAX] {
            let ranks = config.ranks(n);
            assert!(ranks.iter().all(|(_, k)| *k < n));
            assert!(ranks.windows(2).all(|pair| pair[0].1 < pair[1].1));
            assert!(ranks.iter().any(|(r, k)| r == "min" && *k == 0));
            if n > 1 {
                assert!(ranks.iter().any(|(r, k)| r == "max" && *k == n - 1));
            }
        }
        assert!(config.ranks(65536).len() >= 25);
    }

    #[test]
    fn requested_collapsed_quantile_keeps_its_label() {
        let mut config = config("ranks");
        config.ranks = Some(["p01".into()].into());
        assert_eq!(config.ranks(8), vec![("p01".into(), 0)]);
    }

    #[test]
    fn profiles_cover_dispatch_and_cache_sizes() {
        for threshold in [9, 64, 576, 5184, 46656] {
            for n in [threshold - 1, threshold, threshold + 1] {
                assert!(config("thresholds").sizes::<u8>().contains(&n));
            }
        }
        for bytes in CACHE_BYTES {
            assert!(config("cache")
                .sizes::<u64>()
                .contains(&(bytes / size_of::<u64>())));
        }
        assert!(config("thresholds").includes("33-byte", true));
        assert!(!config("ranks").includes("33-byte", true));
    }
}
