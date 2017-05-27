use std::mem::swap;
use std::cmp::max;

fn ceil_div(n: usize, d: usize) -> usize
{
    // n/d + ((n%d != 0) as usize)
    if n != 0 {
        1 + ((n - 1)/ d)
    } else {
        0
    }
}

pub fn kth<T: Ord>(data: &mut [T], k: usize) -> usize
{
    fn select<T: Ord>(d: &mut [T], left: usize, right: usize, n: usize)
        -> usize
    {
        loop {
            if left == right {
                return left;
            }
            let pi = pivot(d, left, right);
            let pi = partition(d, left, right, pi);

            if n == pi {
                return n;
            } else if n < pi {
                right = pi - 1;
            } else {
                left = pi + 1;
            }
        }
    }

    fn partition<T: Ord>(d: &mut [T], left: usize, right: usize, pivot_index: usize)
        -> usize
    {
        let pv = d[pivot_index];
        swap(&mut d[pivot_index], &mut d[right]);
        let mut si = left;
        for i in left..right {
            if d[i] < pv {
                swap(&mut d[si], &mut d[i]);
                si += 1;
            }
        }

        swap(&mut d[right], &mut d[si]);
        si
    }

    fn pivot<T: Ord>(d: &mut [T], left: usize, right: usize)
        -> usize
    {
        if right - left > 5 {
            return partition5(d, left, right);
        }

        let mut i = left;
        loop {
            if i >= right {
                break;
            }

            let sr = max(i + 4, right);
            let m5 = partition5(d, i, sr);
            swap(&mut d[m5], &mut d[left + (i-left)/5]);

            i += 5;
        }

        select(d, left, left + ceil_div(right-left,5)-1,
            left + (right-left)/10)
    }

    // find the median of 5 elements, return the index
    fn partition5<T: Ord>(d: &mut [T], left: usize, right: usize)
        -> usize
    {
        d[left..right].sort();
        3
    }

    0
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut x = [1,2,3];
        assert_eq!(2, x[kth(&mut x, 2)]);
    }
}
