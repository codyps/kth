//! Small driver for perf: setup, validation and reporting stay outside the loop.
//! perf stat includes buffer reset; perf record separates it from selection.
use std::{fmt::Debug, hint::black_box};

#[path = "../benches/support/experiments.rs"]
#[allow(dead_code)]
mod experiments;
#[allow(dead_code)]
#[path = "../benches/inputs.rs"]
mod inputs;
#[path = "../benches/support/numeric_experiments.rs"]
#[allow(dead_code)]
mod numeric_experiments;

fn standard<T: Ord>(a: &mut [T], k: usize) {
    a.select_nth_unstable(k);
}

fn selector<T: Ord>(name: &str) -> fn(&mut [T], usize) {
    match name {
        "kth" => kth::partition_by_kth,
        "std" => standard,
        "control" => experiments::control,
        "block128" => experiments::block128,
        "fr-sample" => experiments::fr_sample,
        "fr-sample-block" => experiments::fr_sample_block,
        "ancestor" => experiments::ancestor,
        "sesquick" => experiments::sesquick,
        "expand" => experiments::expand,
        _ => panic!("unknown generic selector: {name}"),
    }
}

#[inline(never)]
fn run<T: Ord + Clone + Debug>(
    bank: &[Vec<T>],
    k: usize,
    iterations: usize,
    select: fn(&mut [T], usize),
) {
    let mut expected = bank[0].clone();
    expected.sort_unstable();
    let mut values = bank[0].clone();
    select(&mut values, k);
    assert_eq!(values[k], expected[k]);
    assert!(values[..k].iter().all(|v| v <= &values[k]));
    assert!(values[k + 1..].iter().all(|v| v >= &values[k]));
    values.sort_unstable();
    assert_eq!(values, expected);
    let select = black_box(select);
    for i in 0..iterations {
        values.clone_from(&bank[i % bank.len()]);
        select(black_box(&mut values), black_box(k));
        black_box(&values);
    }
    println!(
        "completed {iterations} selections, n={}, k={k}",
        values.len()
    );
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(
        args.len(),
        6,
        "usage: profile_selection ALGORITHM TYPE CASE N RANK ITERATIONS"
    );
    let (algorithm, kind, case) = (args[0].as_str(), args[1].as_str(), args[2].as_str());
    let n: usize = args[3].parse().unwrap();
    let iterations: usize = args[5].parse().unwrap();
    assert!(n > 0 && iterations > 0);
    let k = match args[4].as_str() {
        "min" => 0,
        "max" => n - 1,
        "median" => n / 2,
        "p01" => n / 100,
        "p99" => n / 100 * 99 + n % 100 * 99 / 100,
        _ => args[4].parse().unwrap(),
    };
    assert!(k < n);
    let random = if case == "random" || kind == "String" || kind == "str" {
        inputs::random_bank_of(n, 8, |v| v)
    } else {
        vec![(0..n)
            .map(|i| match case {
                "sorted" => i as u64,
                "reverse" => (n - i) as u64,
                "equal" => 0,
                "sawtooth" => (i % 7) as u64,
                "organ-pipe" => i.min(n - 1 - i) as u64,
                "dominant" if i % 10 != 0 => 5,
                "dominant" => (i as u64).wrapping_mul(6364136223846793005),
                _ => panic!("unsupported numeric case: {case}"),
            })
            .collect()]
    };
    match kind {
        "u8" => {
            let bank: Vec<Vec<u8>> = random
                .iter()
                .map(|a| a.iter().map(|v| *v as u8).collect())
                .collect();
            let select = match algorithm {
                "kth-u8" => kth::partition_u8_by_kth,
                "counting" => numeric_experiments::counting,
                "counting4" => numeric_experiments::counting4,
                "histogram" => numeric_experiments::histogram,
                "histogram4" => numeric_experiments::histogram4,
                _ => selector(algorithm),
            };
            run(&bank, k, iterations, select);
        }
        "u64" => {
            let select = match algorithm {
                "radix8" => numeric_experiments::radix8,
                "radix11" => numeric_experiments::radix11,
                #[cfg(feature = "native-simd")]
                "x86-simd" => numeric_experiments::simd,
                _ => selector(algorithm),
            };
            run(&random, k, iterations, select);
        }
        "64-byte" => run(&inputs::wide(&random), k, iterations, selector(algorithm)),
        "String" | "str" => {
            let words = inputs::strings(case, &random);
            if kind == "String" {
                run(&words, k, iterations, selector(algorithm));
            } else {
                let borrowed: Vec<Vec<&str>> = words
                    .iter()
                    .map(|a| a.iter().map(String::as_str).collect())
                    .collect();
                run(&borrowed, k, iterations, selector(algorithm));
            }
        }
        _ => panic!("unsupported type: {kind}"),
    }
}
