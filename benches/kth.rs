#![cfg(feature = "nightly")]

#![feature(test)]

extern crate test;
extern crate rand;

extern crate kth;

use kth::quickselect;

use rand::Rng;

const BENCH_LEN: usize = 1024 * 512;

#[bench]
fn median_of_medians_big(b: &mut test::Bencher) {
    let mut rng = rand::thread_rng();
    let mut d = vec![0u8; BENCH_LEN];
    b.iter(|| {
        rng.fill_bytes(&mut d);
        let p = rng.gen::<usize>() % d.len();
        quickselect::quickselect(quickselect::median_of_medians, &mut d[..], p)
    })
}

#[bench]
fn repeated_step3_big(b: &mut test::Bencher) {
    let mut rng = rand::thread_rng();
    let mut d = vec![0u8; BENCH_LEN];
    b.iter(|| {
        rng.fill_bytes(&mut d);
        let p = rng.gen::<usize>() % d.len();
        quickselect::quickselect(quickselect::repeated_step3, &mut d[..], p)
    })
}
