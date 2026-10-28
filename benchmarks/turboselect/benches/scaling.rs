use criterion::{criterion_group, criterion_main, Criterion};

#[path = "../../../benches/scaling_cases.rs"]
mod scaling_cases;

fn compare(c: &mut Criterion) {
    for n in scaling_cases::sizes() {
        for (case, input) in scaling_cases::partition_cases(n) {
            let mut group = c.benchmark_group(format!("scaling-experimental/{case}"));
            scaling_cases::bench_partition(&mut group, "kth", &input, kth::partition_by_kth);
            scaling_cases::bench_partition(&mut group, "std", &input, |v, i| {
                v.select_nth_unstable(i);
            });
            scaling_cases::bench_partition(&mut group, "turboselect", &input, |v, i| {
                turboselect::select_nth_unstable(v, i);
            });
            group.finish();
        }
    }
}

criterion_group!(scaling, compare);
criterion_main!(scaling);
