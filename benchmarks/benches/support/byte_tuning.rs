//! Byte histogram experiments with a frozen fbb3235 explicit-byte baseline.
use kth::instrumentation::{self, Phase};
#[path = "byte_before_small.rs"]
mod before_small;
pub use before_small::select as before_small;

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

pub fn lanes1(a: &mut [u8], k: usize) {
    if !eligible(a, k) || a.len() > u32::MAX as usize {
        return baseline(a, k);
    }
    fewer_lanes::<1>(a);
}

pub fn lanes2(a: &mut [u8], k: usize) {
    if !eligible(a, k) || a.len() > u32::MAX as usize {
        return baseline(a, k);
    }
    fewer_lanes::<2>(a);
}

#[inline(never)]
fn fewer_lanes<const LANES: usize>(a: &mut [u8]) {
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

pub fn ordered_bytes(a: &mut [u8], k: usize) {
    if eligible(a, k) {
        let first = a[0];
        let middle = a[a.len() / 2];
        let last = a[a.len() - 1];
        let phase = instrumentation::phase(Phase::Sampling);
        if first <= middle && middle <= last && monotone::<false>(a) {
            return;
        }
        if first >= middle && middle >= last && monotone::<true>(a) {
            instrumentation::moved::<u8>(a.len());
            a.reverse();
            return;
        }
        drop(phase);
    }
    kth::partition_u8_by_kth(a, k);
}

#[inline(never)]
fn monotone<const REVERSE: bool>(a: &[u8]) -> bool {
    a.windows(2).all(|pair| {
        instrumentation::scanned(1);
        if REVERSE {
            pair[0] >= pair[1]
        } else {
            pair[0] <= pair[1]
        }
    })
}

pub fn adaptive_lanes(a: &mut [u8], k: usize) {
    if !eligible(a, k) || a.len() > u16::MAX as usize {
        return baseline(a, k);
    }
    let phase = instrumentation::phase(Phase::Sampling);
    let n = a.len();
    let anchor = a[n / 2];
    let mut equal = 0;
    let mut diversity = 0u32;
    for &v in a[..16]
        .iter()
        .chain(&a[n / 2..n / 2 + 16])
        .chain(&a[n - 16..])
    {
        equal += usize::from(v == anchor);
        diversity |= 1 << (v & 31);
    }
    instrumentation::scanned(48);
    drop(phase);
    if equal >= 32 {
        narrow::<8>(a);
    } else if diversity.count_ones() >= 16 {
        if n <= 2048 {
            fewer_lanes::<1>(a);
        } else {
            fewer_lanes::<2>(a);
        }
    } else {
        baseline(a, k);
    }
}

pub fn sampled_lanes(a: &mut [u8], k: usize) {
    if !eligible(a, k) || a.len().div_ceil(8) > u16::MAX as usize {
        return baseline(a, k);
    }
    if concentrated(a) {
        return narrow::<8>(a);
    }
    if dispersed(a) {
        if a.len() <= 2048 {
            return fewer_lanes::<1>(a);
        }
        return fewer_lanes::<2>(a);
    }
    baseline(a, k);
}

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
fn dispersed(a: &[u8]) -> bool {
    let _phase = instrumentation::phase(Phase::Sampling);
    instrumentation::scanned(9);
    let mut bits = 0u32;
    for &v in &a[..9] {
        bits |= 1 << (v & 31);
    }
    bits.count_ones() >= 8
}
