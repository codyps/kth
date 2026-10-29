#[path = "../benches/support/adaptive.rs"]
mod adaptive;
#[path = "../benches/support/byte_tuning.rs"]
mod byte_tuning;
#[allow(dead_code)]
#[path = "../benches/support/cyclic_baseline.rs"]
mod cyclic_baseline;
#[path = "../benches/support/experiments.rs"]
mod experiments;
#[path = "../benches/support/numeric_experiments.rs"]
mod numeric_experiments;

use rand::{rngs::StdRng, Rng, SeedableRng};

#[test]
fn adaptive_candidates_preserve_partition_contract_on_ordered_and_mixed_inputs() {
    fn check<T: Ord + Clone + std::fmt::Debug>(input: &[T], k: usize) {
        let mut expected = input.to_vec();
        expected.sort_unstable();
        for select in [
            adaptive::adaptive_control,
            adaptive::stratified_mid,
            adaptive::adaptive_block,
            adaptive::stratified_block,
            adaptive::stratified_all,
            adaptive::guarded,
            adaptive::strict_duplicates,
            adaptive::bounded_duplicates,
            adaptive::guarded_cyclic,
            adaptive::combined,
            adaptive::verified_duplicates,
            adaptive::verified_guarded,
            adaptive::verified_control,
            adaptive::verified8,
            adaptive::verified32,
        ] {
            let mut actual = input.to_vec();
            select(&mut actual, k);
            assert_eq!(actual[k], expected[k]);
            assert!(actual[..k].iter().all(|v| v <= &actual[k]));
            assert!(actual[k + 1..].iter().all(|v| v >= &actual[k]));
            actual.sort_unstable();
            assert_eq!(actual, expected);
        }
    }
    let mut rng = StdRng::seed_from_u64(1793250000);
    for n in [65usize, 1024, 4095, 4096, 4097, 8192, 16384, 16385] {
        for pattern in 0..7 {
            let input: Vec<u64> = (0..n)
                .map(|i| match pattern {
                    0 => rng.random(),
                    1 => i as u64,
                    2 => (n - i) as u64,
                    3 => i.min(n - 1 - i) as u64,
                    4 => (i % 7) as u64,
                    5 => 7,
                    _ => {
                        if i % 17 == 0 {
                            rng.random()
                        } else {
                            7
                        }
                    }
                })
                .collect();
            let wide: Vec<_> = input.iter().map(|v| [*v; 8]).collect();
            let strings: Vec<_> = input.iter().map(|v| format!("{v:016x}")).collect();
            let borrowed: Vec<_> = strings.iter().map(String::as_str).collect();
            for k in [0, 1, n / 100, n / 8, n / 2, n - n / 8, n - 2, n - 1] {
                check(&input, k);
                check(&wide, k);
                check(&strings, k);
                check(&borrowed, k);
            }
        }
    }
}

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

#[test]
fn byte_counter_width_and_reconstruction_variants_preserve_every_byte() {
    for n in [
        1usize, 65, 1023, 1024, 1025, 1031, 4096, 65528, 65535, 65536, 65537,
    ] {
        for pattern in 0..6 {
            let input: Vec<u8> = (0..n)
                .map(|i| match pattern {
                    0 => (i.wrapping_mul(7919)) as u8,
                    1 => 255,
                    2 => 0,
                    3 => (i % 7) as u8,
                    4 => {
                        if i % 10 == 0 {
                            i as u8
                        } else {
                            5
                        }
                    }
                    _ => (n - i) as u8,
                })
                .collect();
            let mut expected = input.clone();
            expected.sort_unstable();
            for k in [0, n / 100, n / 2, n - 1] {
                for select in [
                    byte_tuning::baseline,
                    byte_tuning::compact,
                    byte_tuning::lanes8,
                    byte_tuning::merged,
                    byte_tuning::bitmap,
                    byte_tuning::entropy8,
                ] {
                    let mut a = input.clone();
                    select(&mut a, k);
                    assert_eq!(a[k], expected[k]);
                    assert!(a[..k].iter().all(|x| *x <= a[k]));
                    assert!(a[k + 1..].iter().all(|x| *x >= a[k]));
                    a.sort_unstable();
                    assert_eq!(a, expected);
                }
            }
        }
    }
}
