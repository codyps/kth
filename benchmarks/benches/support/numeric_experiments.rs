//! Explicit integer-only benchmark APIs; no dispatch based on generic T size.

pub fn histogram(a: &mut [u8], k: usize) {
    histogram_impl::<false>(a, k);
}
pub fn histogram4(a: &mut [u8], k: usize) {
    histogram_impl::<true>(a, k);
}

// Numeric bytes have no identity beyond their value: the same histogram can
// reconstruct a sorted slice, satisfying the complete partition contract with
// sequential fills instead of unpredictable three-way partition branches.
pub fn counting(a: &mut [u8], k: usize) {
    counting_impl::<false>(a, k);
}
pub fn counting4(a: &mut [u8], k: usize) {
    counting_impl::<true>(a, k);
}

fn counting_impl<const FOUR: bool>(a: &mut [u8], k: usize) {
    assert!(k < a.len());
    let mut counts = [[0usize; 256]; 4];
    if FOUR {
        for chunk in a.as_chunks::<4>().0 {
            for j in 0..4 {
                counts[j][chunk[j] as usize] += 1;
            }
        }
        for v in a.as_chunks::<4>().1 {
            counts[0][*v as usize] += 1;
        }
        let [first, second, third, fourth] = &mut counts;
        for (i, total) in first.iter_mut().enumerate() {
            *total += second[i] + third[i] + fourth[i];
        }
    } else {
        for v in a.iter() {
            counts[0][*v as usize] += 1;
        }
    }
    let mut start = 0;
    for (value, count) in counts[0].iter().enumerate() {
        a[start..start + count].fill(value as u8);
        start += count;
    }
}

fn histogram_impl<const FOUR: bool>(a: &mut [u8], k: usize) {
    assert!(k < a.len());
    let mut counts = [[0usize; 256]; 4];
    if FOUR {
        for chunk in a.as_chunks::<4>().0 {
            for j in 0..4 {
                counts[j][chunk[j] as usize] += 1;
            }
        }
        for v in a.as_chunks::<4>().1 {
            counts[0][*v as usize] += 1;
        }
        let [first, second, third, fourth] = &mut counts;
        for (i, total) in first.iter_mut().enumerate() {
            *total += second[i] + third[i] + fourth[i];
        }
    } else {
        for v in a.iter() {
            counts[0][*v as usize] += 1;
        }
    }
    let mut remaining = k;
    let value = counts[0]
        .iter()
        .position(|count| {
            if remaining < *count {
                true
            } else {
                remaining -= count;
                false
            }
        })
        .unwrap() as u8;
    let (mut lo, mut i, mut hi) = (0, 0, a.len());
    while i < hi {
        if a[i] < value {
            a.swap(lo, i);
            lo += 1;
            i += 1;
        } else if a[i] > value {
            hi -= 1;
            a.swap(i, hi);
        } else {
            i += 1;
        }
    }
}

pub fn radix8(a: &mut [u64], k: usize) {
    radix::<8, 256>(a, k);
}
pub fn radix11(a: &mut [u64], k: usize) {
    radix::<11, 2048>(a, k);
}

fn radix<const BITS: u32, const BUCKETS: usize>(mut a: &mut [u64], mut k: usize) {
    assert!(k < a.len());
    let mut shift = 64 - BITS;
    loop {
        if a.len() <= 64 {
            kth::partition_by_kth(a, k);
            return;
        }
        let mut counts = [0usize; BUCKETS];
        for v in a.iter() {
            counts[((*v >> shift) as usize) & (BUCKETS - 1)] += 1;
        }
        let mut remaining = k;
        let digit = counts
            .iter()
            .position(|count| {
                if remaining < *count {
                    true
                } else {
                    remaining -= count;
                    false
                }
            })
            .unwrap();
        if counts[digit] != a.len() {
            let (mut lo, mut i, mut hi) = (0, 0, a.len());
            while i < hi {
                let d = ((a[i] >> shift) as usize) & (BUCKETS - 1);
                if d < digit {
                    a.swap(lo, i);
                    lo += 1;
                    i += 1;
                } else if d > digit {
                    hi -= 1;
                    a.swap(i, hi);
                } else {
                    i += 1;
                }
            }
            a = &mut a[lo..hi];
            k -= lo;
        }
        if shift == 0 {
            return;
        }
        shift = shift.saturating_sub(BITS);
    }
}

#[cfg(all(feature = "native-simd", kth_native_simd))]
unsafe extern "C" {
    fn kth_experiment_simd_u64(a: *mut u64, n: usize, k: usize);
}

#[cfg(feature = "native-simd")]
pub fn simd_available() -> bool {
    #[cfg(all(target_arch = "x86_64", kth_native_simd))]
    {
        std::is_x86_feature_detected!("avx2")
            && std::is_x86_feature_detected!("popcnt")
            && std::is_x86_feature_detected!("avx512f")
            && std::is_x86_feature_detected!("avx512dq")
            && std::is_x86_feature_detected!("avx512vl")
            && std::is_x86_feature_detected!("avx512bw")
    }
    #[cfg(not(all(target_arch = "x86_64", kth_native_simd)))]
    {
        false
    }
}

#[cfg(feature = "native-simd")]
pub fn simd(a: &mut [u64], k: usize) {
    assert!(k < a.len());
    assert!(
        simd_available(),
        "AVX-512 reference is unsupported on this CPU"
    );
    // SAFETY: the wrapper receives a valid, exclusively borrowed contiguous
    // u64 slice and an in-bounds rank; required CPU features were checked.
    #[cfg(kth_native_simd)]
    unsafe {
        kth_experiment_simd_u64(a.as_mut_ptr(), a.len(), k);
    }
    #[cfg(not(kth_native_simd))]
    {
        let _ = (a, k);
        unreachable!("native reference was not built");
    }
}
