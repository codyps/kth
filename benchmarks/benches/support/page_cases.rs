//! Curated published coverage, shared with the Python report through a manifest.
use crate::inputs;

#[derive(Debug)]
pub struct Case<'a> {
    pub kind: &'a str,
    pub distribution: &'a str,
    pub ranks: Vec<&'a str>,
    pub sizes: Vec<usize>,
}

pub fn cases() -> Vec<Case<'static>> {
    let mut lines = include_str!("../../page_cases.csv").lines();
    assert_eq!(lines.next(), Some("type,distribution,ranks,sizes"));
    lines
        .map(|line| {
            let fields: Vec<_> = line.split(',').collect();
            assert_eq!(fields.len(), 4);
            Case {
                kind: fields[0],
                distribution: fields[1],
                ranks: fields[2].split(';').collect(),
                sizes: fields[3].split(';').map(|n| n.parse().unwrap()).collect(),
            }
        })
        .collect()
}

pub fn rank(label: &str, n: usize) -> usize {
    assert!(n > 0);
    match label {
        "median" => n / 2,
        "p01" => n / 100,
        "p25" => n / 4,
        "p75" => n / 4 * 3 + n % 4 * 3 / 4,
        "p99" => n / 100 * 99 + n % 100 * 99 / 100,
        _ => panic!("unknown page rank: {label}"),
    }
}

pub fn bank<T: Ord>(case: &Case<'_>, n: usize, convert: impl Fn(u64) -> T) -> Vec<Vec<T>> {
    assert!(n > 0);
    if matches!(case.distribution, "random" | "random17") {
        let count = if core::mem::size_of::<T>().saturating_mul(n) >= 64 << 20 {
            4
        } else {
            16
        };
        return inputs::random_bank_of(n, count, |v| {
            convert(if case.distribution == "random17" {
                v % 17
            } else {
                v
            })
        });
    }
    let mut values: Vec<_> = (0..n)
        .map(|i| {
            convert(match case.distribution {
                "ordered-asc" | "ordered-desc" => i as u64,
                "organ-pipe" => i.min(n - 1 - i) as u64,
                "sawtooth" => (i % 7) as u64,
                "periodic17" => (i % 17) as u64,
                "dominant" if i % 10 != 0 => 5,
                "clustered" if i < n * 9 / 10 => 5,
                "dominant" | "clustered" => (i as u64).wrapping_mul(6364136223846793005),
                _ => panic!("unknown page distribution: {}", case.distribution),
            })
        })
        .collect();
    if case.distribution.starts_with("ordered-") {
        // Sort after conversion: byte ramps must not wrap into periodic inputs.
        values.sort_unstable();
        if case.distribution == "ordered-desc" {
            values.reverse();
        }
    }
    vec![values]
}

pub fn bytes<const N: usize>(value: u64) -> [u8; N] {
    std::array::from_fn(|i| value.to_be_bytes()[i % 8])
}
