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

#[cfg(test)]
mod tests {
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
