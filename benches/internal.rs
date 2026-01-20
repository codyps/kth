use criterion::{criterion_group, Criterion};
use rand::distr::uniform::UniformSampler;
use rand::{distr::uniform::UniformUsize, prelude::*};

fn vec_rand() -> Vec<u8> {
    let mut v = vec![0u8; 2048];
    let mut rng = rand::rng();

    rng.fill(&mut v[..]);
    v
}

fn partition5(c: &mut Criterion) {
    c.bench_function("partition5", |b| {
        let mut rng = rand::rng();
        let mut d = [0u8; 5];
        b.iter(|| {
            rng.fill_bytes(&mut d);
            kth::quickselect::partition5(&mut d);
        })
    });
}

fn partition5_2(c: &mut Criterion) {
    c.bench_function("partition5_2", |b| {
        let mut rng = rand::rng();
        let mut d = [0u8; 5];
        b.iter(|| {
            rng.fill_bytes(&mut d);
            kth::quickselect::partition5_2(&mut d);
        })
    });
}

const BENCH_LEN: usize = 1024 * 512;

fn median_of_medians_big(c: &mut Criterion) {
    c.bench_function("median_of_medians_big", |b| {
        let mut rng = rand::rng();
        let mut d = vec![0u8; BENCH_LEN];
        b.iter(|| {
            rng.fill_bytes(&mut d);
            let p = UniformUsize::new(0, d.len()).unwrap().sample(&mut rng);
            kth::quickselect::quickselect(kth::quickselect::bfprt_baseline, &mut d[..], p)
        })
    });
}

fn repeated_step3_big(c: &mut Criterion) {
    c.bench_function("repeated_step3_big", |b| {
        let mut rng = rand::rng();
        let mut d = vec![0u8; BENCH_LEN];
        b.iter(|| {
            rng.fill_bytes(&mut d);
            let p = UniformUsize::new(0, d.len()).unwrap().sample(&mut rng);
            kth::quickselect::quickselect(kth::quickselect::repeated_step3, &mut d[..], p)
        })
    });
}

fn bench_rand_std(c: &mut Criterion) {
    c.bench_function("std rand", |b| {
        let mut v = vec_rand();
        b.iter(|| {
            let i = v.len() / 2;
            v.select_nth_unstable(i);
        })
    });
}

criterion_group!(
    internal,
    partition5,
    partition5_2,
    median_of_medians_big,
    repeated_step3_big,
    bench_rand_std,
);
