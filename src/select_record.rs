//! Large-record selection, gated before entering a separate partition driver.
use super::{choose_pivot as fixed_pivot, partition_equal};
use crate::fixed_record_policy as fixed_select;
use crate::instrumentation::{self, Phase};
use crate::quickselect::{hoare_partition, quickselect, repeated_step3};
use core::{cmp::Ordering, mem::size_of};

// Keep the original selector body and its hot paths intact. The public type
// gate calls this only for records of at least 64 bytes and lengths >= 16384.
#[inline(never)]
pub(crate) fn select<T: Ord>(a: &mut [T], k: usize) {
    let n = a.len();
    assert!(
        k < n,
        "the order index must be smaller than the slice length"
    );
    if n < 16384 || k <= n / 8 || n - 1 - k <= n / 8 {
        fixed_select(a, k);
        return;
    }
    let use_fixed = {
        let _phase = instrumentation::phase(Phase::Sampling);
        let (pivot, repeated) = fixed_pivot(a, k);
        repeated || !is_disordered(a, pivot)
    };
    if use_fixed {
        fixed_select(a, k);
    } else {
        select_with_budget(a, k, n.saturating_mul(4));
    }
}

fn select_with_budget<T: Ord>(mut a: &mut [T], mut k: usize, mut budget: usize) {
    loop {
        let _partition_phase = instrumentation::phase(Phase::Partition);
        let n = a.len();
        if n == 1 {
            return;
        }
        // Extreme ranks need only a scan, including after narrowing the slice.
        if k == 0 {
            instrumentation::scanned(n - 1);
            let p = (1..n).fold(0, |p, i| if a[i] < a[p] { i } else { p });
            instrumentation::swap(a, 0, p);
            return;
        }
        if k == n - 1 {
            instrumentation::scanned(n - 1);
            let p = (1..n).fold(0, |p, i| if a[i] > a[p] { i } else { p });
            instrumentation::swap(a, n - 1, p);
            return;
        }
        // Keep the existing selector for small slices. Charge each sampled pass
        // for its whole active slice, rather than limiting recursion depth:
        // at most 4 * original_len partition elements are processed before
        // the O(n) fallback. Pivot sampling is also at most linear per pass.
        if n <= 64 || budget < n {
            instrumentation::dispatch(n <= 64);
            let _fallback_phase = instrumentation::phase(Phase::Fallback);
            quickselect(repeated_step3, a, k);
            return;
        }
        budget -= n;

        let sampling_phase = instrumentation::phase(Phase::Sampling);
        let (pivot, repeated) = choose_pivot(a, k);
        drop(sampling_phase);
        if repeated {
            // A repeated sample suggests many duplicates. Discard the whole
            // equal band instead of repeatedly selecting among equal elements.
            // For small elements, a low-rank pivot often leaves a mostly-greater
            // suffix. Skip greater/greater swaps there. Keeping the original loop
            // for larger types also keeps their partition driver small.
            let (lo, hi) = if size_of::<T>() <= 8 && k <= n / 8 {
                partition_equal::<_, true>(a, pivot)
            } else {
                partition_equal::<_, false>(a, pivot)
            };
            if k < lo {
                a = &mut a[..lo];
            } else if k >= hi {
                a = &mut a[hi..];
                k -= hi;
            } else {
                return;
            }
            continue;
        }

        // Unconditional swaps avoid an unpredictable branch for moderately sized
        // elements, including String. For larger elements, Hoare moves less data.
        // The entry hint has already rejected predictable distributions.
        // Keep buffering narrowed random tails even when their pivot is skewed.
        let p = if n >= 1024 {
            block_partition(a, pivot)
        } else {
            hoare_partition(a, pivot)
        };
        if p == k {
            return;
        }
        if p > k {
            a = &mut a[..p];
        } else {
            a = &mut a[p + 1..];
            k -= p + 1;
        }
    }
}

#[inline(never)]
fn choose_pivot<T: Ord>(a: &[T], k: usize) -> (usize, bool) {
    let (pivot, repeated) = fixed_pivot(a, k);
    let n = a.len();
    if n >= 16384 && !repeated && k > n / 8 && n - 1 - k > n / 8 && is_disordered(a, pivot) {
        (index_pivot(a, k), false)
    } else {
        (pivot, repeated)
    }
}

// Adjacent predicate changes estimate branch pressure in the actual scan.
// Reset between windows so periodic ramps do not look like random input merely
// because distant probes land on different phases. Equal keys keep the old path.
#[inline(never)]
fn is_disordered<T: Ord>(a: &[T], pivot: usize) -> bool {
    let n = a.len();
    debug_assert!(n >= 48);
    let _phase = instrumentation::phase(Phase::Sampling);
    let mut changes = 0;
    for start in [0, n / 2, n - 16] {
        // Comparing the pivot occurrence with itself tells us nothing about
        // duplicate density. Other equal occurrences reject the hint.
        let first = start + usize::from(start == pivot);
        instrumentation::scanned(1);
        let order = a[first].cmp(&a[pivot]);
        if order == Ordering::Equal {
            return false;
        }
        let mut last = order == Ordering::Less;
        for i in first + 1..start + 16 {
            if i == pivot {
                continue;
            }
            instrumentation::scanned(1);
            let order = a[i].cmp(&a[pivot]);
            if order == Ordering::Equal {
                return false;
            }
            let next = order == Ordering::Less;
            changes += usize::from(next != last);
            last = next;
        }
    }
    changes >= 4
}

// Two 128-byte offset buffers; swaps preserve ownership even on comparison panic.
#[inline(never)]
fn block_partition<T: Ord>(a: &mut [T], pivot: usize) -> usize {
    const B: usize = 128;
    instrumentation::partition_pass();
    instrumentation::swap(a, 0, pivot);
    let (pivot, tail) = a.split_first_mut().unwrap();
    let (mut left, mut right) = (0, tail.len());
    let (mut left_offsets, mut right_offsets) = ([0u8; B], [0u8; B]);
    let (mut nl, mut nr, mut il, mut ir) = (0, 0, 0, 0);
    while right - left >= 2 * B {
        if il == nl {
            nl = 0;
            il = 0;
            instrumentation::scanned(B);
            for i in 0..B {
                left_offsets[nl] = i as u8;
                nl += usize::from(tail[left + i] >= *pivot);
            }
        }
        if ir == nr {
            nr = 0;
            ir = 0;
            instrumentation::scanned(B);
            for i in 0..B {
                right_offsets[nr] = i as u8;
                nr += usize::from(tail[right - 1 - i] < *pivot);
            }
        }
        let pairs = (nl - il).min(nr - ir);
        for j in 0..pairs {
            instrumentation::swap(
                tail,
                left + left_offsets[il + j] as usize,
                right - 1 - right_offsets[ir + j] as usize,
            );
        }
        il += pairs;
        ir += pairs;
        if il == nl {
            left += B;
        }
        if ir == nr {
            right -= B;
        }
    }
    // Rescan the unresolved interval, including any partly consumed block.
    while left < right {
        instrumentation::scanned(1);
        if tail[left] < *pivot {
            left += 1;
            continue;
        }
        while left < right {
            right -= 1;
            if left == right {
                break;
            }
            instrumentation::scanned(1);
            if tail[right] < *pivot {
                instrumentation::swap(tail, left, right);
                left += 1;
                break;
            }
        }
    }
    instrumentation::swap(a, 0, left);
    left
}

// Refine the pivot through indices in a bounded stack sample. The input
// remains read-only throughout sampling, and the original work budget is kept.
#[inline(never)]
fn index_pivot<T: Ord>(a: &[T], k: usize) -> usize {
    let n = a.len();
    let count = n.isqrt().clamp(31, 255) | 1;
    let mut positions = [0usize; 255];
    let width = n / count;
    let mut state = (n as u64) ^ 0x13198a2e03707344;
    for (i, position) in positions[..count].iter_mut().enumerate() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let start = i * width;
        let length = if i + 1 == count { n - start } else { width };
        *position = start + (((state as u128) * (length as u128)) >> 64) as usize;
    }
    let rank = ((k as u128) * (count as u128) / (n as u128)) as usize;
    positions[..count].select_nth_unstable_by(rank, |&i, &j| a[i].cmp(&a[j]));
    positions[rank]
}

#[cfg(test)]
mod tests {
    #[test]
    fn blocks_preserve_partition_and_multiset_across_buffer_boundaries() {
        for n in [1usize, 127, 128, 129, 255, 256, 257, 511, 512, 513, 1025] {
            for pattern in 0..6 {
                let input: Vec<_> = (0..n)
                    .map(|i| match pattern {
                        0 => i,
                        1 => n - i,
                        2 => i.min(n - 1 - i),
                        3 => (i * 7919) % 97,
                        4 => i % 7,
                        _ => 0,
                    })
                    .collect();
                let mut expected = input.clone();
                expected.sort_unstable();
                for pivot in [0, n / 4, n / 2, n - 1] {
                    let mut actual = input.clone();
                    let k = super::block_partition(&mut actual, pivot);
                    assert_eq!(actual[k], input[pivot]);
                    assert!(actual[..k].iter().all(|v| *v < actual[k]));
                    assert!(actual[k + 1..].iter().all(|v| *v >= actual[k]));
                    actual.sort_unstable();
                    assert_eq!(actual, expected);
                }
            }
        }
    }

    #[test]
    fn exhausted_record_budget_retains_the_deterministic_fallback() {
        for n in [65usize, 1024, 16384] {
            let input: Vec<_> = (0..n).map(|i| [((i * 7919) % 97) as u64; 8]).collect();
            let mut expected = input.clone();
            expected.sort_unstable();
            for k in [1, n / 2, n - 2] {
                let mut a = input.clone();
                super::select_with_budget(&mut a, k, 0);
                assert_eq!(a[k], expected[k]);
                assert!(a[..k].iter().all(|v| v <= &a[k]));
                assert!(a[k + 1..].iter().all(|v| v >= &a[k]));
                a.sort_unstable();
                assert_eq!(a, expected);
            }
        }
    }

    #[test]
    fn local_probe_rejects_periodic_ramps_and_accepts_random_order() {
        for n in [16384usize, 16385, 16393, 65536] {
            for period in [5, 7, 17, 256] {
                let a: Vec<_> = (0..n).map(|i| [(i % period) as u64; 8]).collect();
                let (pivot, _) = super::fixed_pivot(&a, n / 2);
                assert!(!super::is_disordered(&a, pivot), "n={n}, period={period}");
            }
            let a: Vec<_> = (0..n)
                .map(|i| [(i as u64).wrapping_mul(6364136223846793005); 8])
                .collect();
            let (pivot, _) = super::fixed_pivot(&a, n / 2);
            assert!(super::is_disordered(&a, pivot));
        }
    }
}
