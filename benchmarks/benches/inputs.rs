//! Shared inputs for focused optimization and cross-library comparisons.
use rand::{rngs::StdRng, Rng, SeedableRng};

const STRING_CASES: &[&str] = &[
    "random",
    "prefix64",
    "prefix256",
    "variable",
    "few-unique",
    "sorted",
    "reverse",
    "equal",
];
const REPORT_STRING_CASES: &[&str] = &["random", "prefix64", "prefix256", "few-unique"];

pub fn string_cases(extended: bool) -> impl Iterator<Item = &'static str> {
    let cases = if extended {
        STRING_CASES
    } else {
        REPORT_STRING_CASES
    };
    cases.iter().copied()
}

pub fn random_bank(n: usize) -> Vec<Vec<u64>> {
    random_bank_of(n, 16, |x| x)
}

pub fn random_bank_of<T>(n: usize, count: usize, mut convert: impl FnMut(u64) -> T) -> Vec<Vec<T>> {
    let mut rng = StdRng::seed_from_u64(42);
    (0..count)
        .map(|_| (0..n).map(|_| convert(rng.random())).collect())
        .collect()
}

pub fn strings(case: &str, random: &[Vec<u64>]) -> Vec<Vec<String>> {
    random
        .iter()
        .map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(i, x)| match case {
                    "random" => format!("{x:016x}"),
                    "prefix64" => format!("{}{x:016x}", "a".repeat(64)),
                    "prefix256" => format!("{}{x:016x}", "a".repeat(256)),
                    "variable" => format!("{x:016x}{}", "b".repeat((x % 128) as usize)),
                    "few-unique" => format!("{}", x % 7),
                    "sorted" => format!("{i:016x}"),
                    "reverse" => format!("{:016x}", values.len() - i),
                    "equal" => String::from("equal"),
                    _ => panic!("unknown string workload: {case}"),
                })
                .collect()
        })
        .collect()
}

pub fn wide(random: &[Vec<u64>]) -> Vec<Vec<[u64; 8]>> {
    random
        .iter()
        .map(|v| v.iter().map(|x| [*x; 8]).collect())
        .collect()
}

/// A globally ordered byte ramp, after narrowing to the byte range.
#[allow(dead_code)] // Only the focused byte benchmarks and profiler use this.
pub fn ordered_bytes(n: usize, reverse: bool) -> Vec<u8> {
    let mut values: Vec<_> = (0..n).map(|i| i as u8).collect();
    values.sort_unstable();
    if reverse {
        values.reverse();
    }
    values
}
