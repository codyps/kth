//! Byte histogram experiments with a frozen fbb3235 explicit-byte baseline.
use kth::instrumentation::{self, Phase};

#[inline]
pub fn baseline(a: &mut [u8], k: usize) {
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

// Benchmark-only counter-width and reconstruction experiments.
fn eligible(a: &[u8], k: usize) -> bool {
    assert!(k < a.len());
    a.len() >= 1024 && k != 0 && k != a.len() - 1
}

impl Counter for u16 {
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

pub fn compact(a: &mut [u8], k: usize) {
    if !eligible(a, k) || a.len() > u16::MAX as usize {
        return baseline(a, k);
    }
    narrow::<4>(a);
}
pub fn lanes8(a: &mut [u8], k: usize) {
    if !eligible(a, k) || a.len() > u16::MAX as usize {
        return baseline(a, k);
    }
    narrow::<8>(a);
}
#[inline(never)]
fn narrow<const LANES: usize>(a: &mut [u8]) {
    let _phase = instrumentation::phase(Phase::Partition);
    let n = a.len();
    let mut counts = [[0u16; 256]; LANES];
    let (chunks, rest) = a.as_chunks::<LANES>();
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

#[inline(always)]
fn merged_counts(a: &[u8]) -> [u32; 256] {
    let mut counts = [[0u32; 256]; 4];
    let (chunks, rest) = a.as_chunks::<4>();
    for c in chunks {
        counts[0][c[0] as usize] += 1;
        counts[1][c[1] as usize] += 1;
        counts[2][c[2] as usize] += 1;
        counts[3][c[3] as usize] += 1;
    }
    for &v in rest {
        counts[0][v as usize] += 1;
    }
    let [first, second, third, fourth] = &mut counts;
    for i in 0..256 {
        first[i] += second[i] + third[i] + fourth[i];
    }
    counts[0]
}
pub fn merged(a: &mut [u8], k: usize) {
    if !eligible(a, k) || a.len() > u32::MAX as usize {
        return baseline(a, k);
    }
    reconstruct_merged::<false>(a);
}
pub fn bitmap(a: &mut [u8], k: usize) {
    if !eligible(a, k) || a.len() > u32::MAX as usize {
        return baseline(a, k);
    }
    reconstruct_merged::<true>(a);
}
#[inline(never)]
fn reconstruct_merged<const BITMAP: bool>(a: &mut [u8]) {
    let _phase = instrumentation::phase(Phase::Partition);
    let n = a.len();
    instrumentation::scanned(n);
    let counts = merged_counts(a);
    let mut remaining = a;
    if BITMAP {
        for group in 0..4 {
            let mut occupied = 0u64;
            for bit in 0..64 {
                occupied |= u64::from(counts[group * 64 + bit] != 0) << bit;
            }
            while occupied != 0 {
                let value = group * 64 + occupied.trailing_zeros() as usize;
                occupied &= occupied - 1;
                let count = counts[value] as usize;
                if count == n {
                    return;
                }
                let (head, tail) = remaining.split_at_mut(count);
                head.fill(value as u8);
                instrumentation::moved::<u8>(count);
                remaining = tail;
            }
        }
    } else {
        for (value, &count) in counts.iter().enumerate() {
            let count = count as usize;
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
}

// More lanes target dependency chains on highly concentrated byte data.
// Sample three separated windows to avoid selecting this from one short run.
pub fn entropy8(a: &mut [u8], k: usize) {
    if eligible(a, k) && a.len() <= u16::MAX as usize {
        let anchor = a[a.len() / 2];
        let first = a[..16].iter().filter(|&&v| v == anchor).count();
        let middle = a[a.len() / 2..a.len() / 2 + 16]
            .iter()
            .filter(|&&v| v == anchor)
            .count();
        let last = a[a.len() - 16..].iter().filter(|&&v| v == anchor).count();
        if first + middle + last >= 36 {
            return narrow::<8>(a);
        }
    }
    baseline(a, k);
}
