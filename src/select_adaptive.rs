//! Adaptive large-input selection, with the established selector as its fixed path.
use crate::instrumentation::{self, Phase};
use crate::quickselect::{hoare_partition, quickselect, repeated_step3};
use crate::select::{
    self as fixed, experimental_equal as partition_equal, experimental_partition as partition,
    experimental_pivot as choose_pivot,
};
use core::cmp::Ordering;
use core::mem::size_of;

const GROWING_SAMPLE_MIN: usize = 16_384;

pub(crate) fn select<T: Ord>(a: &mut [T], k: usize) {
    let n = a.len();
    // The fixed selector retains its original body and inlining decisions.
    // Invalid indices and ZSTs also go through its established checks.
    if n < GROWING_SAMPLE_MIN
        || k >= n
        || size_of::<T>() == 0
        || k == 0
        || k == n - 1
        || (size_of::<T>() > 16 && size_of::<T>() <= 32)
        || (size_of::<T>() <= 32 && (k <= n / 8 || n - 1 - k <= n / 8))
    {
        fixed::select(a, k);
        return;
    }
    let use_fixed = {
        let _sampling = instrumentation::phase(Phase::Sampling);
        let (pivot, repeated) = choose_pivot(a, k);
        repeated || !is_disordered(a, pivot)
    };
    if use_fixed {
        fixed::select(a, k);
    } else {
        select_with_budget(a, k, n.saturating_mul(4));
    }
}

#[inline(never)]
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
        let (pivot, repeated) = choose_adaptive_pivot(a, k);
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
        let p = if size_of::<T>() <= 32 {
            partition(a, pivot)
        } else if n >= GROWING_SAMPLE_MIN && is_disordered(a, pivot) {
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
fn choose_adaptive_pivot<T: Ord>(a: &mut [T], k: usize) -> (usize, bool) {
    let base = choose_pivot(a, k);
    let n = a.len();
    // Scalar tail pivots already remove most of the slice cheaply. Retain them;
    // for large records, a more accurate tail sample can save substantial moves.
    let grow_rank = size_of::<T>() > 32 || (k > n / 8 && n - 1 - k > n / 8);
    if n >= GROWING_SAMPLE_MIN && grow_rank && !base.1 && is_disordered(a, base.0) {
        (growing_pivot(a, k), false)
    } else {
        base
    }
}

// One jittered position per disjoint stratum avoids fixed-stride aliasing on
// periodic inputs. Since count <= sqrt(n)*2 and n >= 16,384, gathering forward
// into a[..count] cannot overwrite any future stratum. All moves are safe swaps.
#[inline(never)]
fn growing_pivot<T: Ord>(a: &mut [T], k: usize) -> usize {
    let n = a.len();
    debug_assert!(n >= GROWING_SAMPLE_MIN);
    let _sampling_scope = instrumentation::sampling_scope();
    let count = (2 * n.isqrt()).min(1024);
    let width = n / count;
    let extra = n % count;
    let mut start = 0;
    let mut state = (n as u64) ^ 0x243f6a8885a308d3;
    for i in 0..count {
        let length = width + usize::from(i < extra);
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let index = start + ((state >> 32) as usize) % length;
        instrumentation::swap(a, i, index);
        start += length;
    }
    // Bias tail samples inward rather than selecting their minimum/maximum.
    let mut rank = ((k as u128) * (count as u128) / (n as u128)) as usize;
    let bias = count.isqrt() / 2;
    if rank < count / 4 {
        rank += bias;
    } else if rank > count - count / 4 {
        rank = rank.saturating_sub(bias);
    }
    rank = rank.clamp(1, count - 2);
    // count <= 1024, so this call cannot grow another sample. Its deterministic
    // fallback also bounds sample selection. The outer partition budget is kept.
    fixed::select(&mut a[..count], rank);
    rank
}

// Monotonic classification has one transition and organ-pipe has two. Require
// at least four changes in 16 probes before paying for sampling or buffering.
#[inline(never)]
fn is_disordered<T: Ord>(a: &[T], pivot: usize) -> bool {
    let _phase = instrumentation::phase(Phase::Sampling);
    let step = a.len() / 17;
    let first = a[step].cmp(&a[pivot]);
    if first == Ordering::Equal {
        return false;
    }
    let mut last = first == Ordering::Less;
    let mut changes = 0;
    for i in 2..=16 {
        let order = a[i * step].cmp(&a[pivot]);
        // A large sample can replace a distinct fixed-sample pivot with a
        // frequent value. Preserve the existing pivot and duplicate handling.
        if order == Ordering::Equal {
            return false;
        }
        let next = order == Ordering::Less;
        changes += usize::from(next != last);
        last = next;
    }
    changes >= 4
}

// Buffer classifications before exchanging large records, avoiding a branch
// on every element. Each 128-entry offset buffer occupies one KiB on 64-bit.
// Outlining keeps the scratch arrays and loops out of the common scalar path.
#[inline(never)]
fn block_partition<T: Ord>(a: &mut [T], pivot: usize) -> usize {
    const B: usize = 128;
    instrumentation::partition_pass();
    instrumentation::swap(a, 0, pivot);
    let (pivot, tail) = a.split_first_mut().unwrap();
    let (mut left, mut right) = (0, tail.len());
    let (mut left_offsets, mut right_offsets) = ([0usize; B], [0usize; B]);
    let (mut nl, mut nr, mut il, mut ir) = (0, 0, 0, 0);
    while right - left >= 2 * B {
        if il == nl {
            nl = 0;
            il = 0;
            instrumentation::scanned(B);
            for i in 0..B {
                left_offsets[nl] = i;
                nl += usize::from(tail[left + i] >= *pivot);
            }
        }
        if ir == nr {
            nr = 0;
            ir = 0;
            instrumentation::scanned(B);
            for i in 0..B {
                right_offsets[nr] = i;
                nr += usize::from(tail[right - 1 - i] < *pivot);
            }
        }
        let pairs = (nl - il).min(nr - ir);
        for j in 0..pairs {
            instrumentation::swap(
                tail,
                left + left_offsets[il + j],
                right - 1 - right_offsets[ir + j],
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

#[cfg(test)]
mod tests {
    use super::*;

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
                    let k = block_partition(&mut actual, pivot);
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
    fn exhausted_adaptive_budget_uses_the_deterministic_fallback() {
        for n in [65usize, 128, 1024] {
            let input: Vec<_> = (0..n).map(|i| (i * 7919) % 97).collect();
            let mut expected = input.clone();
            expected.sort_unstable();
            for k in [1, n / 2, n - 2] {
                let mut actual = input.clone();
                select_with_budget(&mut actual, k, 0);
                assert_eq!(actual[k], expected[k]);
                assert!(actual[..k].iter().all(|v| *v <= actual[k]));
                assert!(actual[k + 1..].iter().all(|v| *v >= actual[k]));
                actual.sort_unstable();
                assert_eq!(actual, expected);
            }
        }
    }
}
