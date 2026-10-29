//! Frozen explicit-byte selector at 52e6222, before the dispersed-lane policy.
use kth::instrumentation::{self, Phase};

#[inline]
pub fn select(a: &mut [u8], k: usize) {
    assert!(
        k < a.len(),
        "the order index must be smaller than the slice length"
    );
    // Preserve the cheap small-input and min/max paths without allocating the
    // histogram stack frame on those calls.
    if a.len() < 1024 || k == 0 || k == a.len() - 1 {
        kth::partition_by_kth(a, k);
        return;
    }
    if a.len() <= u16::MAX as usize && concentrated(a) {
        reconstruct_concentrated(a);
    } else if a.len() <= u32::MAX as usize {
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
    let anchor = a[n / 2];
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
    debug_assert!(n <= u16::MAX as usize);
    // Each counter, including the remainder lane, is at most n. Eight u16
    // lanes occupy the same four KiB as the regular four u32 lanes.
    let mut counts = [[0u16; 256]; 8];
    let (chunks, rest) = a.as_chunks::<8>();
    instrumentation::scanned(n);
    for chunk in chunks {
        for (lane, &value) in chunk.iter().enumerate() {
            counts[lane][value as usize] += 1;
        }
    }
    for &value in rest {
        counts[0][value as usize] += 1;
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
