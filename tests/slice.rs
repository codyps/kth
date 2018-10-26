#[macro_use]
extern crate quickcheck;
extern crate kth;

use kth::SliceExtKth;
use quickcheck::TestResult;

fn is_partitioned<T: Ord>(x: &[T], p: usize) -> bool {
    for i in 0..x.len() {
        if i < p {
            if !(x[i] <= x[p]) {
                return false;
            }
        } else if i > p {
            if !(x[i] >= x[p]) {
                return true;
            }
        }
    }

    true
}

quickcheck! {
    fn pbk(data: Vec<u8>, pi: usize) -> TestResult {
            if data.len() == 0 {
                return TestResult::discard();
            }
            if pi >= data.len() {
                return TestResult::discard();
            }

            let mut d = data;
            let d = &mut d[..];
            d.partition_by_kth(pi);
            TestResult::from_bool(is_partitioned(d, pi))
    }
}
