use kth::quickselect::{bfprt_baseline, quickselect, repeated_step, repeated_step3};
use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};

fn check_selection(input: &[usize], k: usize, partition: fn(&mut [usize]) -> usize) {
    let mut expected = input.to_vec();
    expected.sort_unstable();
    let mut actual = input.to_vec();
    quickselect(partition, &mut actual, k);
    assert_eq!(actual[k], expected[k]);
    assert!(actual[..k].iter().all(|v| *v <= actual[k]));
    assert!(actual[k + 1..].iter().all(|v| *v >= actual[k]));
    actual.sort_unstable();
    assert_eq!(actual, expected);
}

#[test]
fn repeated_step_handles_base_cases_and_recursion() {
    for n in 1..=100 {
        let input: Vec<_> = (0..n).rev().collect();
        for k in [0, n / 2, n - 1] {
            check_selection(&input, k, repeated_step);
        }
    }
}

#[test]
fn repeated_step3_respects_ninther_bounds() {
    let mut input = [8, 7, 6, 5, 0, 4, 3, 1, 2];
    let p = repeated_step3(&mut input);
    assert!((3..=5).contains(&p), "pivot rank: {p}");
}

#[test]
fn bfprt_respects_grouped_median_bounds() {
    let mut input = [
        10, 11, 0, 12, 13, 14, 15, 1, 16, 17, 18, 19, 2, 20, 21, 22, 23, 3, 24, 25, 26, 27, 4, 28,
        29,
    ];
    let p = bfprt_baseline(&mut input);
    assert!((8..=16).contains(&p), "pivot rank: {p}");
}

#[test]
fn pivot_bounds_hold_across_recursive_sample_sizes() {
    let mut rng = StdRng::seed_from_u64(42);
    for n in [9, 81, 729] {
        for _ in 0..100 {
            let mut input: Vec<_> = (0..n).collect();
            input.shuffle(&mut rng);
            let p = repeated_step3(&mut input);
            let medians = n / 9;
            // Each ninther has at least four elements on each side,
            // including itself. The median of those medians gives these rank bounds.
            let lower = 4 * (medians / 2 + 1) - 1;
            let upper = n - 4 * (medians - medians / 2);
            assert!((lower..=upper).contains(&p), "n={n}, pivot rank={p}");
        }
    }
    for n in [25, 125, 625] {
        for _ in 0..100 {
            let mut input: Vec<_> = (0..n).collect();
            input.shuffle(&mut rng);
            let p = bfprt_baseline(&mut input);
            let medians = n / 5;
            let lower = 3 * (medians / 2 + 1) - 1;
            let upper = n - 3 * (medians - medians / 2);
            assert!((lower..=upper).contains(&p), "n={n}, pivot rank={p}");
        }
    }
}

#[test]
fn selectors_match_sorting_for_exhaustive_duplicate_inputs() {
    for n in 1..=9u32 {
        for mut code in 0..3usize.pow(n) {
            let input: Vec<_> = (0..n)
                .map(|_| {
                    let value = code % 3;
                    code /= 3;
                    value
                })
                .collect();
            for k in 0..input.len() {
                for partition in [repeated_step3, repeated_step, bfprt_baseline] {
                    check_selection(&input, k, partition);
                }
            }
        }
    }
}
