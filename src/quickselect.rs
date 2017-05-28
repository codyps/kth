//! Efficiently locate the k-th element of an array. The k-th element is the element that would be
//! at index `k` if the array were sorted.
//!
//! Finding the k-th element is generally faster than sorting the array.
//!
//! Algorithms here are largely based on [SEA 207].
//!
//! [SEA 207](http://erdani.com/research/sea2017.pdf)

/// Find the `k`-th smallest element in `a`, place it at `a[k]` and partition `a` around that
/// element (all smaller than the element to the left, all larger to the right).
///
///  - `partition` is a function that chooses & returns a pivot index (`p`) and partitions `a`
///    around `a[p]`.
///  - `a` is the input array
///  - `k` is the order of the desired element
pub fn quickselect<T: Ord, P: Fn(&mut [T]) -> usize>(partition: P, mut a: &mut [T], mut k: usize)
{
    loop {
        let p = partition(a);
        if p == k {
            return;
        }

        if p > k {
            a = &mut {a}[0..p];
        } else {
            k = k - p - 1;
            a = &mut {a}[(p+1)..];
        }
    }
}

/// Sort 5 elements in-place
///
/// # Implementation
///
/// This is a sorting network for 5 elements.
///
/// This network was taken from
/// http://www.angelfire.com/blog/ronz/Articles/999SortingNetworksReferen.html , which in turn
/// references Knuth's TAOCP volume 3.
///
/// There may exist more efficient non-sorting network sorts that could replace this.
pub fn sort5<T: Ord>(a: &mut [T;5])
{
    let mut cswap = |i: usize, j: usize| {
        if a[i] > a[j] {
            a.swap(i, j)
        }
    };


    cswap(1,2); cswap(3,4);
    cswap(1,3);
    cswap(0,2);
    cswap(2,4);
    cswap(0,3);
    cswap(0,1); cswap(2,3);
    cswap(1,2);
}

/// Find the median and partition around it in a 5 element array.
///
/// # Implementation
/// 
/// Uses a 7 operation (7 comparison, <=7 swap) network. 6 comparison selection algorithms exist,
/// this could be replaced with a more efficient algorithm.
///
/// Essentially the same as a sorting network for 5 elements, but with mixing for the outer 2
/// removed. Saves 2 operations.
pub fn partition5<T: Ord>(a: &mut [T;5])
{
    let mut cswap = |i: usize, j: usize| {
        if a[i] > a[j] {
            a.swap(i, j)
        }
    };

    cswap(0,1); cswap(3,4);
    cswap(0,3); cswap(1,4);
    cswap(2,3);
    cswap(1,2);
    cswap(2,3);
}

/// Sort 3 elements. This is equivalent to a theoretical `partition3`.
///
/// A comparison tree with at most 3 comparisons & 2 swaps.
pub fn sort3<T: Ord>(a: &mut [T;3])
{
    if a[0] <= a[1] {
        if a[1] <= a[2] {
        } else {
            if a[0] <= a[2] {
                a.swap(1,2);
            } else {
                a.swap(0,1);
                a.swap(0,2);
            }
        }
    } else {
        if a[0] <= a[2] {
            a.swap(0,1);
        } else {
            // 1 < 0 && 2 < 0
            if a[1] < a[2] {
                a.swap(0,1);
                a.swap(1,2);
            } else {
                a.swap(0,2);
            }
        }
    }
}

/// median-of-medians on groups of 3 elements
pub fn repeated_step3<T: Ord>(a: &mut [T])
    -> usize
{
    let l = a.len();
    if l < 9 {
        return hoare_partition(a, l/2);
    }

    let mut i = 0;
    let mut j = 0;

    while i + 2 < a.len() {
        sort3(index_fixed!(&mut a;..3));
        a.swap(i+1, j);
        i += 3;
        j += 1;
    }

    let mut i = 0;
    let mut m = 0;
    while i + 2 < j {
        sort3(index_fixed!(&mut a;..3));
        a.swap(i+1, m);
        i += 3;
        m += 1;
    }

    quickselect(repeated_step3, &mut a[..m], m/2);
    hoare_partition(a, m/2)
}

/// Find the median of medians (recursively).
///
/// This can be used as `partition` for `quickselect` (and itself uses `quickselect` internally).
///
/// Does not find the actual median of the array, but finds something in the 30% to 70% bound,
/// which often can serve as a useful pivot point.
///
/// Split the array into 5 element windows, find the find the median & partition each of those
/// windows, then on the array of medians, find the median again using the same method until we
/// have less than 5 elements
pub fn median_of_medians<T: Ord>(a: &mut [T])
    -> usize
{
    let l = a.len();
    if l < 5 {
        return hoare_partition(a, l/2);
    }

    let mut i = 0;
    let mut j = 0;

    while i + 4 < a.len() {
        partition5(index_fixed!(&mut a;..5));
        a.swap(i+2, j);
        i += 5;
        j += 1;
    }

    quickselect(median_of_medians, &mut a[0..j], j/2);
    hoare_partition(a, j/2)
}

/// Partition an array (move all elements greater than a given element to one side, and all
/// elements larger than the same element to the other).
///
/// `pivot` is the index in `arr` of the element to partition around
///
/// Operates in O(n) time.
///
/// # Panics
///
///  - If `pivot` is not a valid index in `arr`.
///  - If `arr.len()` is 0
///
/// # Internal Details
///
/// Moves two cursors (one from left & one from right).
///
///  - Scans from left until a larger than pivot element is found
///  - Scans from right until a smaller than pivot element is found
///  - Swap elements if the cursors have not crossed.
///  - Repeat
pub fn hoare_partition<T: Ord>(arr: &mut [T], pivot: usize)
    -> usize
{
    let p = pivot;
    debug_assert!(arr.len() > 0);
    debug_assert!(p < arr.len());

    arr.swap(0, p);
    let mut a = 1;
    let mut b = arr.len() - 1;

    'a: loop {
        loop {
            if a > b {
                break 'a;
            }

            if arr[a] >= arr[0] {
                break;
            }

            a += 1;
        }

        while arr[0] < arr[b] {
            b -= 1;
        }

        if a >= b {
            break;
        }

        arr.swap(a,b);
        a += 1;
        b -= 1;
    }

    a -= 1;
    arr.swap(0,a);
    a
}

#[cfg(test)]
mod test {
    use quickcheck::TestResult;

    fn is_sorted<T: Ord>(a: &[T]) -> bool {
        for w in a.windows(2) {
            if w[0] > w[1] {
                return false;
            }
        }

        true
    }

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
        fn sort5(d: Vec<u8>) -> TestResult {
            let mut d = d;
            if d.len() < 5 {
                return TestResult::discard();
            }
            let d = index_fixed!(&mut d;..5);
            super::sort5(d);
            TestResult::from_bool(is_sorted(d))
        }

        fn partition5(d: Vec<u8>) -> TestResult {
            let mut d = d;
            if d.len() < 5 {
                return TestResult::discard();
            }
            let d = index_fixed!(&mut d;..5);
            super::partition5(d);

            TestResult::from_bool(is_partitioned(d, 3))
        }

        fn sort3(d: Vec<u8>) -> TestResult {
            let mut d = d;
            if d.len() < 3 {
                return TestResult::discard();
            }
            let d = index_fixed!(&mut d;..3);
            super::sort3(d);
            if !is_sorted(d) {
                println!("{}:{}: {:?}", file!(), line!(), d);
            }
            TestResult::from_bool(is_sorted(d))
        }
    }

    fn check_hp(x: &mut [u8], pivot: usize) -> Result<usize,String> {
        let op = x[pivot];
        let p = super::hoare_partition(&mut x[..], pivot);
        if !(op == x[p]) {
            return Err(format!("{}:{}: Check failed: {} == {}", file!(), line!(), op, x[p]));
        }
        if !is_partitioned(x, p) {
            return Err(format!("{}:{}: not partitioned", file!(), line!()));
        }

        return Ok(p);
    }

    #[test]
    fn hoare_partition() {
        let mut x = [3, 1, 2, 4, 5];
        let p = check_hp(&mut x[..], 0).unwrap();
        assert_eq!(p, 2);

        let mut x = [1,2,3,4,5];
        let p = check_hp(&mut x[..], 2).unwrap();
        assert_eq!(p, 2);
    }

    quickcheck!{
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

}
