#[path = "../benches/support/before_followup.rs"]
mod before;
#[path = "../benches/support/ordered_hint.rs"]
mod ordered;
use rand::{rngs::StdRng, Rng, SeedableRng};
fn check<T: Ord + Clone + std::fmt::Debug>(input: &[T], k: usize) {
    let mut expected = input.to_vec();
    expected.sort_unstable();
    for select in [before::select, ordered::select] {
        let mut a = input.to_vec();
        select(&mut a, k);
        assert_eq!(a[k], expected[k]);
        assert!(a[..k].iter().all(|v| v <= &a[k]));
        assert!(a[k + 1..].iter().all(|v| v >= &a[k]));
        a.sort_unstable();
        assert_eq!(a, expected);
    }
}
#[test]
fn ordered_pivots_preserve_contract_at_thresholds_and_on_misleading_samples() {
    let mut rng = StdRng::seed_from_u64(2026102901);
    for n in [65usize, 1023, 1024, 1025, 4096, 16384, 65535, 65536, 65537] {
        for pattern in 0..7 {
            let mut a: Vec<u64> = (0..n)
                .map(|i| match pattern {
                    0 => rng.random(),
                    1 => i as u64,
                    2 => (n - i) as u64,
                    3 => (i % 7) as u64,
                    4 => 5,
                    5 => {
                        if i % 10 == 0 {
                            rng.random()
                        } else {
                            5
                        }
                    }
                    _ => (n - i) as u64,
                })
                .collect();
            if pattern == 6 {
                // A deceptive ordered probe must only bias a pivot, never bypass partitioning.
                let mut state = (n as u64) ^ 0xa4093822299f31d0;
                for j in 0..16 {
                    state = state
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    let start = j * (n / 16);
                    let width = if j == 15 { n - start } else { n / 16 };
                    let index =
                        start + ((((state >> 32) as u128) * (width as u128)) >> 32) as usize;
                    a[index] = j as u64;
                }
            }
            for k in [1, n / 100, n / 8, n / 2, n * 99 / 100, n - 2] {
                check(&a, k);
            }
            if n <= 4096 {
                let owned: Vec<_> = a.iter().map(|v| Box::new(*v)).collect();
                check(&owned, n / 2);
            }
        }
    }
}
