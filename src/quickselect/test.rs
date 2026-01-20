use quickcheck::quickcheck;
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

fn check_hp(x: &mut [u8], pivot: usize) -> Result<usize, String> {
    let op = x[pivot];
    let p = super::hoare_partition(&mut x[..], pivot);
    if !(op == x[p]) {
        return Err(format!(
            "{}:{}: Check failed: {} == {}",
            file!(),
            line!(),
            op,
            x[p]
        ));
    }
    if !is_partitioned(x, p) {
        return Err(format!("{}:{}: not partitioned", file!(), line!()));
    }

    return Ok(p);
}

quickcheck! {
    fn sort5(d: Vec<u8>) -> TestResult {
        let mut d = d;
        if d.len() < 5 {
            return TestResult::discard();
        }
        let d = &mut d[..5];
        super::sort5(d.try_into().unwrap());
        TestResult::from_bool(d.is_sorted())
    }

    fn partition5(d: Vec<u8>) -> TestResult {
        let mut d = d;
        if d.len() < 5 {
            return TestResult::discard();
        }
        let d = &mut d[..5];
        super::partition5(d.try_into().unwrap());

        TestResult::from_bool(is_partitioned(d, 3))
    }

    fn partition5_2(d: Vec<u8>) -> TestResult {
        let mut d = d;
        if d.len() < 5 {
            return TestResult::discard();
        }
        let d = &mut d[..5];
        super::partition5_2(d.try_into().unwrap());

        TestResult::from_bool(is_partitioned(d, 3))
    }

    fn sort3(d: Vec<u8>) -> TestResult {
        let mut d = d;
        if d.len() < 3 {
            return TestResult::discard();
        }
        let d = &mut d[..3];
        super::sort3(d.try_into().unwrap());
        if !d.is_sorted() {
            println!("{}:{}: {:?}", file!(), line!(), d);
        }
        TestResult::from_bool(d.is_sorted())
    }

    fn qs_median_of_medians(d: Vec<u8>, po: usize) -> TestResult {
        let mut d = d;
        if d.len() == 0 {
            return TestResult::discard();
        }
        if po >= d.len() {
            return TestResult::discard();
        }

        super::quickselect(super::bfprt_baseline, &mut d[..], po);
        TestResult::from_bool(is_partitioned(&mut d[..], po))
    }

    fn qs_repeated_step3(d: Vec<u8>, po: usize) -> TestResult {
        let mut d = d;
        if d.len() == 0 {
            return TestResult::discard();
        }
        if po >= d.len() {
            return TestResult::discard();
        }

        super::quickselect(super::repeated_step3, &mut d[..], po);
        TestResult::from_bool(is_partitioned(&mut d[..], po))
    }

    fn hoare_partition_qc(data: Vec<u8>, pos: usize) -> TestResult {
        let mut d = data;
        if d.len() == 0 {
            return TestResult::discard();
        }
        if pos >= d.len() {
            return TestResult::discard();
        }
        TestResult::from_bool(check_hp(&mut d[..], pos).is_ok())
    }
}
