#[macro_use]
extern crate quickcheck;
extern crate kth;

use kth::SliceExtKth;
use quickcheck::TestResult;

fn is_partitioned<T: Ord>(x: &[T], p: usize) -> bool {
    x[..p].iter().all(|v| v <= &x[p]) && x[p + 1..].iter().all(|v| v >= &x[p])
}

#[test]
fn partition_oracle_checks_both_sides() {
    assert!(!is_partitioned(&[2, 1, 3], 1));
    assert!(!is_partitioned(&[0, 2, 1], 1));
    assert!(is_partitioned(&[1, 2, 2, 3], 2));
}

quickcheck! {
    fn pbk(data: Vec<u8>, pi: usize) -> TestResult {
            if data.is_empty() {
                return TestResult::discard();
            }
            if pi >= data.len() {
                return TestResult::discard();
            }

            let mut expected = data.clone();
            expected.sort_unstable();
            let mut d = data;
            d.partition_by_kth(pi);
            if d[pi] != expected[pi] || !is_partitioned(&d, pi) {
                return TestResult::failed();
            }
            d.sort_unstable();
            TestResult::from_bool(d == expected)
    }
}
