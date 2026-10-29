//! Optional operation counts. Timing benchmarks must leave the feature disabled.

#[derive(Clone, Copy)]
pub enum Phase {
    Other,
    Sampling,
    Partition,
    Fallback,
}

#[cfg(feature = "bench-instrumentation")]
mod enabled {
    use super::Phase;
    use std::cell::Cell;

    #[derive(Clone, Copy, Debug, Default)]
    pub struct Counts {
        pub comparisons: u64,
        pub sampling_comparisons: u64,
        pub partition_comparisons: u64,
        pub fallback_comparisons: u64,
        pub elements_scanned: u64,
        pub bytes_moved: u64,
        pub partition_passes: u64,
        pub small_slice_dispatches: u64,
        pub budget_fallbacks: u64,
        /// Positive when scan/movement totals exclude an opaque library sort.
        pub opaque_sort_calls: u64,
    }

    std::thread_local! {
        static COUNTS: Cell<Counts> = Cell::new(Counts::default());
        static PHASE: Cell<Phase> = const { Cell::new(Phase::Other) };
    }

    pub fn reset() {
        COUNTS.with(|c| c.set(Counts::default()));
        PHASE.with(|p| p.set(Phase::Other));
    }

    pub fn snapshot() -> Counts {
        COUNTS.with(Cell::get)
    }

    pub(super) fn update(f: impl FnOnce(&mut Counts)) {
        COUNTS.with(|cell| {
            let mut counts = cell.get();
            f(&mut counts);
            cell.set(counts);
        });
    }

    pub fn comparison() {
        update(|c| {
            c.comparisons += 1;
            PHASE.with(|p| match p.get() {
                Phase::Other => (),
                Phase::Sampling => c.sampling_comparisons += 1,
                Phase::Partition => c.partition_comparisons += 1,
                Phase::Fallback => c.fallback_comparisons += 1,
            });
        });
    }

    pub struct PhaseGuard(Phase);

    impl Drop for PhaseGuard {
        fn drop(&mut self) {
            PHASE.with(|p| p.set(self.0));
        }
    }

    pub fn phase(phase: Phase) -> PhaseGuard {
        PhaseGuard(PHASE.with(|p| p.replace(phase)))
    }
}

#[cfg(feature = "bench-instrumentation")]
pub use enabled::{comparison, phase, reset, snapshot, Counts, PhaseGuard};

#[cfg(not(feature = "bench-instrumentation"))]
pub struct PhaseGuard;

#[cfg(not(feature = "bench-instrumentation"))]
impl Drop for PhaseGuard {
    #[inline(always)]
    fn drop(&mut self) {}
}

#[cfg(not(feature = "bench-instrumentation"))]
#[inline(always)]
pub fn phase(_: Phase) -> PhaseGuard {
    PhaseGuard
}

#[inline(always)]
pub fn scanned(n: usize) {
    #[cfg(feature = "bench-instrumentation")]
    enabled::update(|c| c.elements_scanned += n as u64);
    #[cfg(not(feature = "bench-instrumentation"))]
    let _ = n;
}

/// Logical value transfers, including temporary storage and self-copies.
/// This is an algorithmic count, not measured memory traffic.
#[inline(always)]
pub fn moved<T>(n: usize) {
    #[cfg(feature = "bench-instrumentation")]
    enabled::update(|c| c.bytes_moved += n as u64 * core::mem::size_of::<T>() as u64);
    #[cfg(not(feature = "bench-instrumentation"))]
    let _ = n;
}

#[inline(always)]
pub fn swap<T>(a: &mut [T], i: usize, j: usize) {
    moved::<T>(3);
    a.swap(i, j);
}

#[inline(always)]
pub fn partition_pass() {
    #[cfg(feature = "bench-instrumentation")]
    enabled::update(|c| c.partition_passes += 1);
}

#[inline(always)]
pub fn opaque_sort() {
    #[cfg(feature = "bench-instrumentation")]
    enabled::update(|c| c.opaque_sort_calls += 1);
}

#[inline(always)]
pub fn dispatch(small: bool) {
    #[cfg(feature = "bench-instrumentation")]
    enabled::update(|c| {
        if small {
            c.small_slice_dispatches += 1;
        } else {
            c.budget_fallbacks += 1;
        }
    });
    #[cfg(not(feature = "bench-instrumentation"))]
    let _ = small;
}
