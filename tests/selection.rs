use kth::partition_by_kth;
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};
use std::{cell::Cell, cmp::Ordering, rc::Rc};

fn check<T: Ord + Clone + std::fmt::Debug>(input: &[T], k: usize) {
    let mut sorted = input.to_vec();
    sorted.sort_unstable();
    let mut actual = input.to_vec();
    partition_by_kth(&mut actual, k);
    assert_eq!(actual[k], sorted[k]);
    assert!(actual[..k].iter().all(|x| x <= &actual[k]));
    assert!(actual[k + 1..].iter().all(|x| x >= &actual[k]));
    actual.sort_unstable();
    assert_eq!(actual, sorted);
}

#[test]
fn every_rank_matches_sorting_across_cutoffs() {
    let mut rng = StdRng::seed_from_u64(731);
    for n in (1..=66).chain([80, 81, 82, 127, 128, 129, 256]) {
        let mut input: Vec<_> = (0..n as u64).collect();
        for _ in 0..8 {
            input.shuffle(&mut rng);
            for k in 0..n {
                check(&input, k);
                check(&input.iter().map(|v| v % 3).collect::<Vec<_>>(), k);
            }
        }
    }
}

#[test]
fn every_rank_matches_sorting_for_exhaustive_small_inputs() {
    for n in 1..=9u32 {
        for mut code in 0..3usize.pow(n) {
            let input: Vec<_> = (0..n)
                .map(|_| {
                    let value = (code % 3) as u64;
                    code /= 3;
                    value
                })
                .collect();
            let wider: Vec<_> = input.iter().map(|value| [*value; 2]).collect();
            for k in 0..input.len() {
                check(&input, k);
                check(&wider, k);
            }
        }
    }
}

#[test]
fn structured_and_random_inputs_match_sorting() {
    let mut rng = StdRng::seed_from_u64(2718);
    for n in [65, 128, 1024, 8192] {
        let mut leading = vec![0; n];
        leading[0] = 1;
        let mut trailing = vec![0; n];
        trailing[n - 1] = 1;
        let mut inputs = vec![
            (0..n as u64).collect(),
            (0..n as u64).rev().collect(),
            (0..n).map(|i| i.min(n - 1 - i) as u64).collect(),
            (0..n).map(|i| (i % 7) as u64).collect(),
            vec![0; n],
            leading,
            trailing,
        ];
        for _ in 0..16 {
            inputs.push((0..n).map(|_| rng.random()).collect());
            inputs.push((0..n).map(|_| u64::from(rng.random::<u8>())).collect());
        }
        for input in inputs {
            for k in [0, 1, n / 100, n / 4, n / 2, n - 2, n - 1] {
                check(&input, k);
            }
        }
    }
}

#[test]
fn tail_ranks_match_sorting_across_sampling_thresholds() {
    let mut rng = StdRng::seed_from_u64(20261028);
    for n in [65usize, 66, 127, 128, 255, 256, 1024, 4096] {
        let mut ranks = vec![0, n / 4, n / 2, n - 1];
        for boundary in [n / 84, n / 8] {
            for k in [boundary.saturating_sub(1), boundary, boundary + 1] {
                ranks.extend([k, n - 1 - k]);
            }
        }
        ranks.sort_unstable();
        ranks.dedup();
        let inputs = [
            (0..n as u64).collect::<Vec<_>>(),
            (0..n as u64).rev().collect(),
            (0..n).map(|i| i.min(n - 1 - i) as u64).collect(),
            (0..n).map(|_| rng.random()).collect(),
            (0..n).map(|_| u64::from(rng.random::<u8>())).collect(),
            (0..n).map(|i| (i % 3) as u64).collect(),
        ];
        for input in inputs {
            for &k in &ranks {
                check(&input, k);
            }
        }
    }
}

#[test]
fn large_non_copy_elements_preserve_payloads() {
    let input: Vec<_> = (0..1024)
        .map(|i| format!("{:04}:{i}", (i * 37) % 101))
        .collect();
    let mut sorted = input.clone();
    sorted.sort_unstable();
    for k in [0, 1, 255, 512, 1023] {
        let mut actual = input.clone();
        partition_by_kth(&mut actual, k);
        assert_eq!(actual[k], sorted[k]);
        assert!(actual[..k].iter().all(|x| x <= &actual[k]));
        assert!(actual[k + 1..].iter().all(|x| x >= &actual[k]));
        actual.sort_unstable();
        assert_eq!(actual, sorted);
    }
}

#[derive(Eq)]
struct Counted {
    key: usize,
    comparisons: Rc<Cell<usize>>,
}

impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Ord for Counted {
    fn cmp(&self, other: &Self) -> Ordering {
        self.comparisons.set(self.comparisons.get() + 1);
        self.key.cmp(&other.key)
    }
}

impl PartialOrd for Counted {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[test]
fn adversarial_patterns_keep_comparison_counts_bounded() {
    for n in [1024, 4096, 16384, 65536] {
        for pattern in 0..5 {
            let comparisons = Rc::new(Cell::new(0));
            let mut input: Vec<_> = (0..n)
                .map(|i| Counted {
                    key: match pattern {
                        0 => i.min(n - 1 - i),
                        1 => i % 7,
                        2 => n - 1 - i,
                        3 => {
                            if i % 2 == 0 {
                                i / 2
                            } else {
                                n - 1 - i / 2
                            }
                        }
                        _ => 0,
                    },
                    comparisons: comparisons.clone(),
                })
                .collect();
            partition_by_kth(&mut input, n / 2);
            assert!(
                comparisons.get() < 128 * n,
                "n={n}, pattern={pattern}, comparisons={}",
                comparisons.get()
            );
            assert!(input[..n / 2].iter().all(|x| x.key <= input[n / 2].key));
            assert!(input[n / 2 + 1..].iter().all(|x| x.key >= input[n / 2].key));
        }
    }
}

#[test]
fn zero_sized_values() {
    let mut values = [(); 1024];
    partition_by_kth(&mut values, 512);
}

#[test]
#[should_panic]
fn empty_slice_panics() {
    partition_by_kth(&mut [] as &mut [u8], 0);
}

#[test]
#[should_panic]
fn out_of_bounds_panics() {
    partition_by_kth(&mut [1, 2, 3], 3);
}

#[test]
#[should_panic]
fn zero_sized_out_of_bounds_panics() {
    partition_by_kth(&mut [(); 3], 3);
}

#[test]
fn comparator_panics_preserve_owned_elements() {
    struct State {
        calls: Cell<usize>,
        drops: Cell<usize>,
        panic_at: usize,
    }
    struct Item<const PAD: usize> {
        _padding: [u8; PAD],
        key: usize,
        id: usize,
        state: Rc<State>,
    }
    impl<const PAD: usize> PartialEq for Item<PAD> {
        fn eq(&self, other: &Self) -> bool {
            self.key == other.key
        }
    }
    impl<const PAD: usize> Eq for Item<PAD> {}
    impl<const PAD: usize> Ord for Item<PAD> {
        fn cmp(&self, other: &Self) -> Ordering {
            let calls = self.state.calls.get();
            self.state.calls.set(calls + 1);
            assert_ne!(calls, self.state.panic_at, "intentional comparator panic");
            self.key.cmp(&other.key)
        }
    }
    impl<const PAD: usize> PartialOrd for Item<PAD> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl<const PAD: usize> Drop for Item<PAD> {
        fn drop(&mut self) {
            self.state.drops.set(self.state.drops.get() + 1);
        }
    }
    impl<const PAD: usize> AsRef<Item<PAD>> for Item<PAD> {
        fn as_ref(&self) -> &Item<PAD> {
            self
        }
    }
    fn exercise<T: Ord + AsRef<Item<PAD>>, const PAD: usize>(wrap: impl Fn(Item<PAD>) -> T) {
        for n in [8, 16, 64, 256, 16384] {
            for equal in [false, true] {
                for panic_at in [0, 1, 8, 32, 128, 4096, 16384]
                    .into_iter()
                    .filter(|&at| n >= 16384 || (n == 256 && at <= 128) || at < n / 2)
                {
                    let state = Rc::new(State {
                        calls: Cell::new(0),
                        drops: Cell::new(0),
                        panic_at,
                    });
                    let mut values: Vec<_> = (0..n)
                        .map(|id| {
                            wrap(Item {
                                _padding: [0; PAD],
                                key: if equal {
                                    0
                                } else if n >= 16384 {
                                    (id as u64).wrapping_mul(6364136223846793005) as usize
                                } else {
                                    (id * 37) % 257
                                },
                                id,
                                state: state.clone(),
                            })
                        })
                        .collect();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        partition_by_kth(&mut values, n / 2);
                    }));
                    assert!(result.is_err());
                    let mut ids: Vec<_> = values.iter().map(|v| v.as_ref().id).collect();
                    ids.sort_unstable();
                    assert_eq!(ids, (0..n).collect::<Vec<_>>());
                    assert_eq!(state.drops.get(), 0);
                    drop(values);
                    assert_eq!(state.drops.get(), n);
                }
            }
        }
    }
    exercise::<_, 0>(|item| item);
    exercise::<_, 128>(|item| item);
    exercise::<_, 0>(Box::new);
}

#[test]
fn strings_match_sorting_across_cutoffs_and_patterns() {
    let mut rng = StdRng::seed_from_u64(314159);
    for n in [
        1, 2, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 66, 127, 128, 129, 575, 576, 577, 1024,
    ] {
        let random: Vec<_> = (0..n)
            .map(|i| format!("{:016x}{}", rng.random::<u64>(), "x".repeat(i % 31)))
            .collect();
        let prefix: Vec<_> = (0..n)
            .map(|_| format!("{}{:016x}", "語🙂".repeat(32), rng.random::<u64>()))
            .collect();
        let mut sorted = random.clone();
        sorted.sort_unstable();
        let mut reverse = sorted.clone();
        reverse.reverse();
        let mut almost_sorted = sorted.clone();
        almost_sorted.swap(0, n - 1);
        let mut duplicates: Vec<_> = (0..n).map(|i| format!("value {}", i % 7)).collect();
        duplicates.shuffle(&mut rng);
        let mut outlier = vec![String::from("aaa"); n];
        outlier[n - 1] = String::from("bbb");
        let ranks: Vec<_> = if n <= 129 {
            (0..n).collect()
        } else {
            vec![0, 1, n / 100, n / 4, n / 2, n - 2, n - 1]
        };
        for input in [
            random,
            prefix,
            sorted,
            reverse,
            almost_sorted,
            duplicates,
            outlier,
            vec![String::from("same"); n],
        ] {
            let mut expected = input.clone();
            expected.sort_unstable();
            for &k in &ranks {
                let mut actual = input.clone();
                partition_by_kth(&mut actual, k);
                assert_eq!(actual[k], expected[k]);
                assert!(actual[..k].iter().all(|x| x <= &actual[k]));
                assert!(actual[k + 1..].iter().all(|x| x >= &actual[k]));
                actual.sort_unstable();
                assert_eq!(actual, expected);
            }
        }
    }
}

#[test]
fn guarded_samples_and_blocks_preserve_contract_across_thresholds() {
    let mut rng = StdRng::seed_from_u64(20261029);
    // Include the activation threshold, the 1024-sample cap, and irregular tails.
    for n in [16383usize, 16384, 16385, 65537, 262143, 262144, 262145] {
        for pattern in 0..5 {
            let input: Vec<u64> = (0..n)
                .map(|i| match pattern {
                    0 => rng.random(),
                    1 => (i % 7) as u64,
                    2 => i as u64,
                    3 => (n - i) as u64,
                    _ => {
                        if i % 10 == 0 {
                            rng.random()
                        } else {
                            5
                        }
                    }
                })
                .collect();
            let wide: Vec<_> = input.iter().map(|v| [*v; 8]).collect();
            for k in [1, n / 100, n / 8, n / 2, n * 99 / 100, n - 2] {
                check(&input, k);
                check(&wide, k);
            }
        }
    }
}
