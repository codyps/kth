use std::fmt::Debug;

fn check<T: Ord + Clone + Debug>(input: &[T], k: usize) {
    let mut expected = input.to_vec();
    expected.sort_unstable();
    let mut actual = input.to_vec();
    kth::partition_by_kth(&mut actual, k);
    assert_eq!(actual[k], expected[k]);
    assert!(actual[..k].iter().all(|v| v <= &actual[k]));
    assert!(actual[k + 1..].iter().all(|v| v >= &actual[k]));
    actual.sort_unstable();
    assert_eq!(actual, expected);
}

#[test]
fn wide_records_cover_entry_thresholds_and_misleading_windows() {
    for n in [16383usize, 16384, 16385, 65536] {
        for pattern in 0..4 {
            let mut input: Vec<[u64; 16]> = (0..n)
                .map(|i| {
                    let value = match pattern {
                        0 => (i as u64).wrapping_mul(6364136223846793005),
                        1 | 3 => (n - 1 - i) as u64,
                        _ => (i % 17) as u64,
                    };
                    [value; 16]
                })
                .collect();
            if pattern == 3 {
                // The short windows suggest disorder in an otherwise reversed
                // input. The hint must still partition under the work budget.
                for start in [0, n / 2, n - 16] {
                    for j in 0..16 {
                        let value = if j % 2 == 0 { j as u64 } else { (n + j) as u64 };
                        input[start + j] = [value; 16];
                    }
                }
            }
            for k in [n / 5, n / 2, n * 4 / 5] {
                check(&input, k);
            }
        }
    }
}

#[test]
fn sampled_records_preserve_owned_payloads() {
    let input: Vec<[Box<u64>; 8]> = (0..16384u64)
        .map(|i| {
            let value = i.wrapping_mul(6364136223846793005);
            std::array::from_fn(|_| Box::new(value))
        })
        .collect();
    for k in [3276, 8192, 13107] {
        check(&input, k);
    }
}
