//! Explicit byte selection: bounded histograms and sequential reconstruction.
use crate::instrumentation::{self, Phase};

#[inline]
pub(crate) fn select(a: &mut [u8], k: usize) {
    assert!(
        k < a.len(),
        "the order index must be smaller than the slice length"
    );
    // Preserve the cheap small-input and min/max paths without allocating the
    // histogram stack frame on those calls.
    if a.len() < 1024 || k == 0 || k == a.len() - 1 {
        crate::select::select(a, k);
        return;
    }
    if a.len().div_ceil(8) <= u16::MAX as usize {
        if concentrated(a) {
            reconstruct_concentrated(a);
            return;
        }
        if dispersed(a) {
            if a.len() <= 2048 {
                reconstruct_dispersed::<1>(a);
            } else {
                reconstruct_dispersed::<2>(a);
            }
            return;
        }
    }
    if a.len() <= u32::MAX as usize {
        reconstruct::<u32>(a);
    } else {
        reconstruct::<usize>(a);
    }
}

trait Counter: Copy {
    const ZERO: Self;
    fn increment(&mut self);
    fn value(self) -> usize;
}

impl Counter for u32 {
    const ZERO: Self = 0;
    #[inline(always)]
    fn increment(&mut self) {
        *self += 1;
    }
    #[inline(always)]
    fn value(self) -> usize {
        self as usize
    }
}

impl Counter for usize {
    const ZERO: Self = 0;
    #[inline(always)]
    fn increment(&mut self) {
        *self += 1;
    }
    #[inline(always)]
    fn value(self) -> usize {
        self
    }
}

#[inline(never)]
fn reconstruct<C: Counter>(a: &mut [u8]) {
    let _phase = instrumentation::phase(Phase::Partition);
    let n = a.len();
    let mut counts = [[C::ZERO; 256]; 4];
    let (chunks, remainder) = a.as_chunks::<4>();
    instrumentation::scanned(n);
    // Separate counters reduce loop-carried dependencies on low-cardinality
    // data, including long runs of a single value.
    for chunk in chunks {
        counts[0][chunk[0] as usize].increment();
        counts[1][chunk[1] as usize].increment();
        counts[2][chunk[2] as usize].increment();
        counts[3][chunk[3] as usize].increment();
    }
    for &value in remainder {
        counts[0][value as usize].increment();
    }
    let mut remaining = a;
    for (value, &first) in counts[0].iter().enumerate() {
        let count = first.value()
            + counts[1][value].value()
            + counts[2][value].value()
            + counts[3][value].value();
        if count == n {
            // All bytes were equal; the input already satisfies every rank.
            return;
        }
        if count != 0 {
            let (head, tail) = remaining.split_at_mut(count);
            head.fill(value as u8);
            instrumentation::moved::<u8>(count);
            remaining = tail;
        }
    }
}

// The four-lane loop is effective for dispersed values. Additional lanes
// shorten dependent counter-update chains when most input bytes are identical.
#[inline(never)]
fn concentrated(a: &[u8]) -> bool {
    let _phase = instrumentation::phase(Phase::Sampling);
    instrumentation::scanned(48);
    let n = a.len();
    let m = n / 2;
    // Prefer a repeated neighbor so one outlier at the middle does not hide a
    // dominant value. All three reads are inside the middle sample window.
    let anchor = if a[m] == a[m + 1] || a[m] == a[m + 2] {
        a[m]
    } else {
        a[m + 1]
    };
    let first = a[..16].iter().filter(|&&v| v == anchor).count();
    let middle = a[n / 2..n / 2 + 16]
        .iter()
        .filter(|&&v| v == anchor)
        .count();
    let last = a[n - 16..].iter().filter(|&&v| v == anchor).count();
    first + middle + last >= 36
}

#[inline(never)]
fn reconstruct_concentrated(a: &mut [u8]) {
    let _phase = instrumentation::phase(Phase::Partition);
    let n = a.len();
    debug_assert!(n.div_ceil(8) <= u16::MAX as usize);
    // Each lane receives at most ceil(n / 8) bytes, including its remainder.
    // Eight u16 lanes occupy the same four KiB as the regular four u32 lanes.
    let mut counts = [[0u16; 256]; 8];
    let (chunks, rest) = a.as_chunks::<8>();
    instrumentation::scanned(n);
    for chunk in chunks {
        for (lane, &value) in chunk.iter().enumerate() {
            counts[lane][value as usize] += 1;
        }
    }
    for (lane, &value) in rest.iter().enumerate() {
        counts[lane][value as usize] += 1;
    }
    let mut remaining = a;
    for value in 0..256 {
        let count = counts.iter().map(|lane| lane[value] as usize).sum();
        if count == n {
            return;
        }
        if count != 0 {
            let (head, tail) = remaining.split_at_mut(count);
            head.fill(value as u8);
            instrumentation::moved::<u8>(count);
            remaining = tail;
        }
    }
}

// A short prefix hint separates dispersed bytes from small alphabets and runs.
// Collisions in the low five bits make this conservative; a misleading sample
// changes only the counting kernel, which still scans every byte.
#[inline(never)]
fn dispersed(a: &[u8]) -> bool {
    let _phase = instrumentation::phase(Phase::Sampling);
    instrumentation::scanned(9);
    let mut bits = 0u32;
    for &value in &a[..9] {
        bits |= 1 << (value & 31);
    }
    bits.count_ones() >= 8
}

#[inline(never)]
fn reconstruct_dispersed<const LANES: usize>(a: &mut [u8]) {
    let _phase = instrumentation::phase(Phase::Partition);
    let n = a.len();
    let mut counts = [[0u32; 256]; LANES];
    let (chunks, rest) = a.as_chunks::<LANES>();
    instrumentation::scanned(n);
    for chunk in chunks {
        for (lane, &value) in chunk.iter().enumerate() {
            counts[lane][value as usize] += 1;
        }
    }
    for (lane, &value) in rest.iter().enumerate() {
        counts[lane][value as usize] += 1;
    }
    let mut remaining = a;
    for value in 0..256 {
        let count = counts.iter().map(|lane| lane[value] as usize).sum();
        if count == n {
            return;
        }
        if count != 0 {
            let (head, tail) = remaining.split_at_mut(count);
            head.fill(value as u8);
            instrumentation::moved::<u8>(count);
            remaining = tail;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn narrow_lanes_preserve_counts_at_the_limit_and_all_remainders() {
        let limit = (u16::MAX as usize).saturating_mul(8);
        for n in (1024..1032).chain(65528..=65535).chain(limit - 7..=limit) {
            for pattern in 0..3 {
                let mut a: Vec<u8> = (0..n)
                    .map(|i| match pattern {
                        0 => 255,
                        1 => {
                            if i % 10 == 0 {
                                i as u8
                            } else {
                                5
                            }
                        }
                        _ => i as u8,
                    })
                    .collect();
                let mut expected = a.clone();
                expected.sort_unstable();
                super::reconstruct_concentrated(&mut a);
                assert_eq!(a, expected);
            }
        }
    }

    #[test]
    fn a_single_middle_outlier_does_not_hide_concentration() {
        for offset in 0..3 {
            let mut a = vec![5u8; 1024];
            a[512 + offset] = 255;
            assert!(super::concentrated(&a));
        }
    }

    #[test]
    fn dispersed_lanes_preserve_counts_even_on_misleading_samples() {
        for n in (1024..1032).chain(2047..=2049) {
            for pattern in 0..4 {
                let input: Vec<u8> = (0..n)
                    .map(|i| match pattern {
                        0 => (i * 7919) as u8,
                        1 => 255,
                        2 => (i % 7) as u8,
                        _ if i < 9 => i as u8,
                        _ => 5,
                    })
                    .collect();
                let mut expected = input.clone();
                expected.sort_unstable();
                for kernel in [
                    super::reconstruct_dispersed::<1>,
                    super::reconstruct_dispersed::<2>,
                ] {
                    let mut a = input.clone();
                    kernel(&mut a);
                    assert_eq!(a, expected);
                }
            }
        }
    }

    #[test]
    fn wide_histogram_preserves_every_byte_and_remainder() {
        for n in [1024, 1025, 1026, 1027] {
            let mut values: Vec<u8> = (0..n).map(|i| (i * 37) as u8).collect();
            let mut expected = values.clone();
            expected.sort_unstable();
            super::reconstruct::<usize>(&mut values);
            assert_eq!(values, expected);
        }
    }
}
