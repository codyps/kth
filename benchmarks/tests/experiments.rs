#[path = "../benches/support/experiments.rs"]
mod experiments;
#[path = "../benches/support/numeric_experiments.rs"]
mod numeric_experiments;

use rand::{rngs::StdRng, Rng, SeedableRng};

#[test]
fn generic_variants_preserve_the_complete_partition_contract() {
    let mut rng = StdRng::seed_from_u64(19071975);
    let variants: [fn(&mut [u64], usize); 15] = [
        experiments::control,
        experiments::small16,
        experiments::small32,
        experiments::fr_window,
        experiments::expand,
        experiments::block64,
        experiments::block128,
        experiments::ancestor,
        experiments::ancestor_sample,
        experiments::sesquick,
        experiments::fr_block,
        experiments::ninther_all,
        experiments::fr_sample,
        experiments::fr_sample_block,
        experiments::network16,
    ];
    for n in [
        1usize, 2, 3, 4, 5, 6, 7, 8, 9, 16, 32, 63, 64, 65, 127, 128, 129, 255, 256, 257, 600, 601,
        1024, 4096,
    ] {
        for pattern in 0..8 {
            let input: Vec<u64> = (0..n)
                .map(|i| match pattern {
                    0 => i as u64,
                    1 => (n - i) as u64,
                    2 => (i % 7) as u64,
                    3 => 1,
                    4 => {
                        if i % 11 == 0 {
                            rng.random()
                        } else {
                            5
                        }
                    }
                    5 => i.min(n - 1 - i) as u64,
                    6 => u64::from(rng.random::<u8>()),
                    _ => rng.random(),
                })
                .collect();
            let mut expected = input.clone();
            expected.sort_unstable();
            for k in [
                0,
                1.min(n - 1),
                n / 100,
                n / 8,
                n / 2,
                n.saturating_sub(2),
                n - 1,
            ] {
                for (variant, select) in variants.iter().enumerate() {
                    let mut actual = input.clone();
                    select(&mut actual, k);
                    assert_eq!(
                        actual[k], expected[k],
                        "variant={variant}, n={n}, k={k}, pattern={pattern}"
                    );
                    assert!(actual[..k].iter().all(|v| *v <= actual[k]));
                    assert!(actual[k + 1..].iter().all(|v| *v >= actual[k]));
                    actual.sort_unstable();
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}

#[test]
fn generic_variants_handle_non_copy_payloads() {
    for select in [
        experiments::fr_window,
        experiments::expand,
        experiments::block64,
        experiments::block128,
        experiments::ancestor,
        experiments::ancestor_sample,
        experiments::sesquick,
        experiments::small16,
        experiments::small32,
        experiments::fr_block,
        experiments::ninther_all,
        experiments::fr_sample,
        experiments::fr_sample_block,
        experiments::network16,
    ] {
        let input: Vec<String> = (0..2048)
            .map(|i| format!("{:04}-{i}", (i * 37) % 91))
            .collect();
        let mut expected = input.clone();
        expected.sort_unstable();
        for k in [1, 20, 1024, 2027, 2046] {
            let mut actual = input.clone();
            select(&mut actual, k);
            assert_eq!(actual[k], expected[k]);
            assert!(actual[..k].iter().all(|v| v <= &actual[k]));
            assert!(actual[k + 1..].iter().all(|v| v >= &actual[k]));
            actual.sort_unstable();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn numeric_variants_match_sorting_on_duplicates_and_extremes() {
    let mut rng = StdRng::seed_from_u64(256);
    for n in [1, 2, 16, 63, 64, 65, 128, 257, 1024, 65536] {
        for pattern in 0..5 {
            let input: Vec<u64> = (0..n)
                .map(|i| match pattern {
                    0 => rng.random(),
                    1 => (i % 7) as u64,
                    2 => u64::MAX,
                    3 => (i as u64) << 48,
                    _ => u64::from(i % 2 == 0) * u64::MAX,
                })
                .collect();
            let mut expected = input.clone();
            expected.sort_unstable();
            for k in [0, n / 100, n / 2, n - 1] {
                for select in [numeric_experiments::radix8, numeric_experiments::radix11] {
                    let mut actual = input.clone();
                    select(&mut actual, k);
                    assert_eq!(actual[k], expected[k]);
                    assert!(actual[..k].iter().all(|v| *v <= actual[k]));
                    assert!(actual[k + 1..].iter().all(|v| *v >= actual[k]));
                    actual.sort_unstable();
                    assert_eq!(actual, expected);
                }
                let bytes: Vec<u8> = input.iter().map(|v| *v as u8).collect();
                let mut expected_bytes = bytes.clone();
                expected_bytes.sort_unstable();
                for select in [
                    numeric_experiments::histogram,
                    numeric_experiments::histogram4,
                    numeric_experiments::counting,
                    numeric_experiments::counting4,
                ] {
                    let mut actual = bytes.clone();
                    select(&mut actual, k);
                    assert_eq!(actual[k], expected_bytes[k]);
                    assert!(actual[..k].iter().all(|v| *v <= actual[k]));
                    assert!(actual[k + 1..].iter().all(|v| *v >= actual[k]));
                    actual.sort_unstable();
                    assert_eq!(actual, expected_bytes);
                }
                #[cfg(feature = "native-simd")]
                if numeric_experiments::simd_available() {
                    let mut actual = input.clone();
                    numeric_experiments::simd(&mut actual, k);
                    assert_eq!(actual[k], expected[k]);
                    assert!(actual[..k].iter().all(|v| *v <= actual[k]));
                    assert!(actual[k + 1..].iter().all(|v| *v >= actual[k]));
                    actual.sort_unstable();
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}
