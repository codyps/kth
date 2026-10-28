use core::hint::black_box;
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use rand::prelude::*;

fn bench_selector(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    algorithm: &str,
    case: &str,
    input: &[u8],
    select: impl Fn(&mut [u8], usize),
) {
    let i = input.len() / 2;
    let mut expected = input.to_vec();
    expected.sort_unstable();
    let mut actual = input.to_vec();
    select(&mut actual, i);
    assert_eq!(actual[i], expected[i]);
    assert!(actual[..i].iter().all(|x| *x <= actual[i]));
    assert!(actual[i + 1..].iter().all(|x| *x >= actual[i]));
    actual.sort_unstable();
    assert_eq!(actual, expected);

    group.bench_function(BenchmarkId::new(algorithm, case), |b| {
        b.iter_batched_ref(
            || input.to_vec(),
            |v| {
                select(v, i);
                black_box(v);
            },
            BatchSize::SmallInput,
        )
    });
}

fn compare(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(42);
    let mut random = vec![0u8; 8192];
    rng.fill(&mut random[..]);
    let mut ends_with_one = vec![0u8; 4196];
    ends_with_one[4195] = 1;
    let mut starts_with_one = vec![0u8; 4196];
    starts_with_one[0] = 1;

    let mut group = c.benchmark_group("turboselect-comparison");
    for (case, input) in [
        ("rand fixed", random),
        ("[0]*4195+[1]", ends_with_one),
        ("[1]+[0]*4195", starts_with_one),
    ] {
        bench_selector(&mut group, "kth", case, &input, kth::partition_by_kth);
        bench_selector(&mut group, "std", case, &input, |v, i| {
            v.select_nth_unstable(i);
        });
        bench_selector(&mut group, "turboselect", case, &input, |v, i| {
            turboselect::select_nth_unstable(v, i);
        });
    }
    group.finish();
}

criterion_group!(comparisons, compare);
criterion_main!(comparisons);
