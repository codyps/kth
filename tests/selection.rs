use kth::partition_by_kth;
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};
use std::{cell::Cell, cmp::Ordering, rc::Rc};

fn check(input: &[u64], k: usize) {
    let mut sorted = input.to_vec();
    sorted.sort_unstable();
    let mut actual = input.to_vec();
    partition_by_kth(&mut actual, k);
    assert_eq!(actual[k], sorted[k]);
    assert!(actual[..k].iter().all(|x| *x <= actual[k]));
    assert!(actual[k + 1..].iter().all(|x| *x >= actual[k]));
    actual.sort_unstable();
    assert_eq!(actual, sorted);
}

#[test]
fn every_rank_matches_sorting_across_cutoffs() {
    let mut rng = StdRng::seed_from_u64(731);
    for n in [1, 2, 8, 16, 63, 64, 65, 66, 80, 81, 82, 127, 128, 129, 256] {
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
    struct Item {
        key: usize,
        id: usize,
        calls: Rc<Cell<usize>>,
        drops: Rc<Cell<usize>>,
        panic_at: usize,
    }
    impl PartialEq for Item {
        fn eq(&self, other: &Self) -> bool {
            self.key == other.key
        }
    }
    impl Eq for Item {}
    impl Ord for Item {
        fn cmp(&self, other: &Self) -> Ordering {
            let calls = self.calls.get();
            self.calls.set(calls + 1);
            assert_ne!(calls, self.panic_at, "intentional comparator panic");
            self.key.cmp(&other.key)
        }
    }
    impl PartialOrd for Item {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Drop for Item {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    impl AsRef<Item> for Item {
        fn as_ref(&self) -> &Item {
            self
        }
    }
    fn exercise<T: Ord + AsRef<Item>>(wrap: impl Fn(Item) -> T) {
        for equal in [false, true] {
            for panic_at in [0, 1, 8, 32, 128] {
                let drops = Rc::new(Cell::new(0));
                let calls = Rc::new(Cell::new(0));
                let mut values: Vec<_> = (0..256)
                    .map(|id| {
                        wrap(Item {
                            key: if equal { 0 } else { (id * 37) % 257 },
                            id,
                            calls: calls.clone(),
                            drops: drops.clone(),
                            panic_at,
                        })
                    })
                    .collect();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    partition_by_kth(&mut values, 128);
                }));
                assert!(result.is_err());
                let mut ids: Vec<_> = values.iter().map(|v| v.as_ref().id).collect();
                ids.sort_unstable();
                assert_eq!(ids, (0..256).collect::<Vec<_>>());
                assert_eq!(drops.get(), 0);
                drop(values);
                assert_eq!(drops.get(), 256);
            }
        }
    }
    exercise(|item| item);
    exercise(Box::new);
}
