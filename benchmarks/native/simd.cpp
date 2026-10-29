// The SIMD implementation is external, pinned by build.rs, and BSD-3-Clause.
#include "x86simdsort-static-incl.h"

extern "C" void kth_experiment_simd_u64(uint64_t *a, size_t n, size_t k) {
    avx512_qselect(a, k, n, false, false, false);
}
