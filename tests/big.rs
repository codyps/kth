extern crate rand;

extern crate kth;

use kth::quickselect;

use rand::Rng;

const BENCH_LEN: usize = 1024 * 1024 * 4;

#[test]
fn median_of_medians() {
    let mut rng = rand::thread_rng();
    let mut d = vec![0u8; BENCH_LEN];
    for _ in 0..20 {
        rng.fill_bytes(&mut d);
        let p = rng.gen::<usize>() % d.len();
        quickselect::quickselect(quickselect::median_of_medians, &mut d[..], p)
    }
}
