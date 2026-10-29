use kth::partition_u8_by_kth;
use rand::{rngs::StdRng, Rng, SeedableRng};

#[test]
fn bytes_match_sorting_across_cutoffs_and_distributions() {
    let mut rng = StdRng::seed_from_u64(256);
    for n in [
        1usize, 2, 3, 8, 16, 63, 64, 65, 255, 256, 257, 1023, 1024, 1025, 4095, 4096, 4097, 65536,
    ] {
        for pattern in 0..8 {
            let input: Vec<u8> = (0..n)
                .map(|i| match pattern {
                    0 => rng.random(),
                    1 => i as u8,
                    2 => (n - i) as u8,
                    3 => (i % 7) as u8,
                    4 => 0,
                    5 => 255,
                    6 => {
                        if i % 17 == 0 {
                            rng.random()
                        } else {
                            128
                        }
                    }
                    _ => u8::from(i == n - 1),
                })
                .collect();
            let mut expected = input.clone();
            expected.sort_unstable();
            let ranks: Vec<_> = if n <= 65 {
                (0..n).collect()
            } else {
                vec![0, 1, n / 100, n / 4, n / 2, n - 2, n - 1]
            };
            for k in ranks {
                let mut actual = input.clone();
                partition_u8_by_kth(&mut actual, k);
                assert_eq!(actual[k], expected[k], "n={n}, k={k}, pattern={pattern}");
                assert!(actual[..k].iter().all(|v| *v <= actual[k]));
                assert!(actual[k + 1..].iter().all(|v| *v >= actual[k]));
                actual.sort_unstable();
                assert_eq!(actual, expected);
            }
        }
    }
}

#[test]
#[should_panic]
fn empty_bytes_panic() {
    partition_u8_by_kth(&mut [], 0);
}

#[test]
#[should_panic]
fn invalid_byte_rank_panics() {
    partition_u8_by_kth(&mut [1, 2, 3], 3);
}
