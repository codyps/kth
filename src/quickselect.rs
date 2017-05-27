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

/// This is a sorting network for 5 elements.
///
/// This network was taken from
/// http://www.angelfire.com/blog/ronz/Articles/999SortingNetworksReferen.html , which in turn
/// references Knuth's TAOCP volume 3.
fn median5<T: Ord>(a: &mut [T])
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

///
/*
fn median5_sa(a: &mut [T])
{
    if a[0] < a[1] {
        if a[2] < a[3] {
            if a[1] < a[2] {

}
*/

pub fn median_of_medians<T: Ord>(a: &mut [T])
    -> usize
{
    let l = a.len();
    if l < 5 {
        hoare_partition(a, l/2);
    }

    let mut i = 0;
    let mut j = 0;

    while i + 4 < a.len() {
        median5(a);
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
/// *Panics*
///
///  - If `pivot` is not a valid index in `arr`.
///  - If `arr.len()` is 0
///
/// *Internal Details*
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

    fn check_hp(x: &mut [u8], pivot: usize) -> Result<usize,()> {
        let op = x[pivot];
        let p = super::hoare_partition(&mut x[..], pivot);
        if !(op == x[p]) {
            println!("{}:{}: Check failed: {} == {}", file!(), line!(), op, x[p]);
            return Err(());
        }
        for i in 0..x.len() {
            if i < p {
                // FIXME: consider if the == is to permisive in this check, do all of the ==
                // numbers need to be adjacent to the pivot? Or is non-adjacency allowed?
                if !(x[i] <= x[p]) {
                    println!("{}:{}: Check failed: {} < {}", file!(), line!(), x[i], x[p]);
                    return Err(());
                }
            } else if i > p {
                if !(x[i] >= x[p]) {
                    println!("{}:{}: Check failed: {} > {}", file!(), line!(), x[i], x[p]);
                    return Err(());
                }
            }
        }

        return Ok(p);
    }

    #[test]
    fn hp() {
        let mut x = [3, 1, 2, 4, 5];
        let p = check_hp(&mut x[..], 0).unwrap();
        assert_eq!(p, 2);

        let mut x = [1,2,3,4,5];
        let p = check_hp(&mut x[..], 2).unwrap();
        assert_eq!(p, 2);
    }

    quickcheck!{
        fn qc_hp(data: Vec<u8>, pos: usize) -> bool {
            let mut d = data;
            if d.len() == 0 {
                return true;
            }
            let p = if pos > 0 { pos % d.len() } else { 0 };
            check_hp(&mut d[..], p).is_ok()
        }
    }

}
