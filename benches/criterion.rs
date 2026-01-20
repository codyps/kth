use criterion::criterion_main;

mod internal;
mod kth_cmp;

criterion_main!(
    kth_cmp::starts_with_1,
    kth_cmp::ends_with_1,
    kth_cmp::rand,
    internal::internal,
    kth_cmp::rand_fixed
);
