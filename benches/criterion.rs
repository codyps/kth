use criterion::criterion_main;

mod internal;
mod kth_cmp;
mod median_only;

criterion_main!(
    kth_cmp::starts_with_1,
    kth_cmp::ends_with_1,
    kth_cmp::rand,
    internal::internal,
    kth_cmp::rand_fixed,
    median_only::median_only
);
