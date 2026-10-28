use rand::prelude::*;

#[path = "sizes.rs"]
mod sizes;
pub use sizes::sizes;

pub fn partition_cases(n: usize) -> [(&'static str, Vec<u8>); 3] {
    let mut rng = StdRng::seed_from_u64(42);
    let mut random = vec![0u8; n];
    rng.fill(&mut random[..]);
    let mut trailing = vec![0u8; n];
    trailing[n - 1] = 1;
    let mut leading = vec![0u8; n];
    leading[0] = 1;
    [
        ("random-u8", random),
        ("trailing-one", trailing),
        ("leading-one", leading),
    ]
}

pub fn bench_partition<T: Ord + Clone + core::fmt::Debug>(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    algorithm: &str,
    input: &[T],
    select: impl Fn(&mut [T], usize),
) {
    let i = input.len() / 2;
    let mut expected = input.to_vec();
    expected.sort_unstable();
    let mut actual = input.to_vec();
    select(&mut actual, i);
    assert_eq!(actual[i], expected[i]);
    assert!(actual[..i].iter().all(|x| x <= &actual[i]));
    assert!(actual[i + 1..].iter().all(|x| x >= &actual[i]));
    actual.sort_unstable();
    assert_eq!(actual, expected);

    group.bench_function(criterion::BenchmarkId::new(algorithm, input.len()), |b| {
        b.iter_batched_ref(
            || input.to_vec(),
            |v| {
                select(v, i);
                core::hint::black_box(v);
            },
            criterion::BatchSize::LargeInput,
        )
    });
}
