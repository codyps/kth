/// `partition` returns a pivot index (`p`) and partitions `a` around `a[p]`
/// `a` is the input array
/// `k` is the order of the desired element
fn quickselect<T: Ord, P: Fn(&mut [T]) -> usize>(partition: P, mut a: &mut [T], mut k: usize)
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

/// partition an array in `O(n)`.
pub fn hoare_partition<T: Ord>(arr: &mut [T], p: usize)
    -> usize
{
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
