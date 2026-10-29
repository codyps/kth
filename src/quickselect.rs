//! Efficiently locate the k-th element of an array. The k-th element is the element that would be
//! at index `k` if the array were sorted.
//!
//! Finding the k-th element is generally faster than sorting the array.
//!
//! Algorithms here are largely based on [SEA 207].
//!
//! [SEA 207](http://erdani.com/research/sea2017.pdf)
//!
//! Another paper by Andrei Alexandrescu on this topic: https://arxiv.org/pdf/1606.00484v1.pdf

use crate::instrumentation;

/// Find the `k`-th smallest element in `a`, place it at `a[k]` and partition `a` around that
/// element (all smaller than the element to the left, all larger to the right).
///
///  - `partition` is a function that chooses & returns a pivot index (`p`) and partitions `a`
///    around `a[p]`.
///  - `a` is the input array
///  - `k` is the order of the desired element
pub fn quickselect<T: Ord, P: Fn(&mut [T]) -> usize>(partition: P, mut a: &mut [T], mut k: usize) {
    loop {
        let p = partition(a);
        if p == k {
            return;
        }

        if p > k {
            a = &mut { a }[0..p];
        } else {
            k = k - p - 1;
            a = &mut { a }[(p + 1)..];
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
pub fn sort5<T: Ord>(a: &mut [T; 5]) {
    let mut cswap = |i: usize, j: usize| {
        if a[i] > a[j] {
            instrumentation::swap(a, i, j)
        }
    };

    cswap(1, 2);
    cswap(3, 4);
    cswap(1, 3);
    cswap(0, 2);
    cswap(2, 4);
    cswap(0, 3);
    cswap(0, 1);
    cswap(2, 3);
    cswap(1, 2);
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
pub fn partition5_2<T: Ord>(a: &mut [T; 5]) {
    let mut cswap = |i: usize, j: usize| {
        if a[i] > a[j] {
            instrumentation::swap(a, i, j)
        }
    };

    cswap(0, 1);
    cswap(3, 4);
    cswap(0, 3);
    cswap(1, 4);
    cswap(2, 3);
    cswap(1, 2);
    cswap(2, 3);
}

/// Find the median of 5 elements & partition the other 4 elements around the median.
///
/// # Implementation
///
/// From [Alexandrescu] "Algorithm 4: MEDIAN5".
///
/// - <=6 comparisons
/// - <=7 swaps
///
/// [Alexandrescu](https://arxiv.org/pdf/1606.00484v1.pdf)
pub fn partition5<T: Ord>(x: &mut [T; 5]) {
    // a b c d e
    // 0 1 2 3 4
    let a = 0;
    let b = 1;
    let c = 2;
    let d = 3;
    let e = 4;
    if x[c] < x[a] {
        instrumentation::swap(x, a, c);
    }
    if x[d] < x[b] {
        instrumentation::swap(x, b, d);
    }
    if x[d] < x[c] {
        instrumentation::swap(x, c, d);
        instrumentation::swap(x, a, b);
    }
    if x[e] < x[b] {
        instrumentation::swap(x, b, e);
    }
    if x[e] < x[c] {
        instrumentation::swap(x, c, e);
        if x[c] < x[a] {
            instrumentation::swap(x, a, c);
        }
    } else if x[c] < x[b] {
        instrumentation::swap(x, b, c);
    }
}

// Consider a min/max & value based approach for median5/partition5. This will likely speed up
// processing when T = u8 or other integers.
/*
#[cfg(not(m5v2))]
pub fn median5<T: Ord>(a: &mut [T;5])
{
    partition5(a)

    }
    */

/// Accept 3 indexes into x, return the index with the median
pub fn median3_index<T: Ord>(r: &[T], mut a: usize, b: usize, mut c: usize) -> usize {
    if r[a] > r[c] {
        core::mem::swap(&mut a, &mut c);
    }
    if r[b] > r[c] {
        return c;
    }
    if r[b] < r[a] {
        return a;
    }

    b
}

// Accept 3 indexes into `r`, update `r` so that the median is at index `b`.
pub fn median3<T: Ord>(r: &mut [T], a: usize, b: usize, c: usize) {
    if r[b] < r[a] {
        // b < a
        if r[b] < r[c] {
            // b < a, b < c
            if r[c] < r[a] {
                // b < c < a
                instrumentation::swap(r, b, c);
            } else {
                // b < a <= c
                instrumentation::swap(r, b, a);
            }
        }
    } else if r[c] < r[b] {
        // a <= b, c < b
        if r[c] < r[a] {
            // c < a <= b
            instrumentation::swap(r, b, a);
        } else {
            // a <= c < b
            instrumentation::swap(r, b, c);
        }
    }
    debug_assert!(r[a] <= r[b] && r[b] <= r[c] || r[a] >= r[b] && r[b] >= r[c]);
}

/*
#[cfg(m5v2)]
pub fn median5<T: Ord>(a: &mut [T;5])
    -> T
{
    use ::core::cmp::{min,max};
    let f = max(min(a[0], a[1]), min(a[2], a[3]));
    let g = min(max(a[0], a[1]), min(a[2], a[3]));
    median3([f,g,a[4]])
}
    */

/// Sort 3 elements. This is equivalent to a theoretical `partition3`.
///
/// A comparison tree with at most 3 comparisons & 2 swaps.
pub fn sort3<T: Ord>(a: &mut [T; 3]) {
    if a[0] <= a[1] {
        if a[1] <= a[2] {
        } else {
            if a[0] <= a[2] {
                instrumentation::swap(a, 1, 2);
            } else {
                instrumentation::swap(a, 0, 1);
                instrumentation::swap(a, 0, 2);
            }
        }
    } else {
        if a[0] <= a[2] {
            instrumentation::swap(a, 0, 1);
        } else {
            // 1 < 0 && 2 < 0
            if a[1] < a[2] {
                instrumentation::swap(a, 0, 1);
                instrumentation::swap(a, 1, 2);
            } else {
                instrumentation::swap(a, 0, 2);
            }
        }
    }
}

/// Accept 9 indexes into `r`, swaps the ninther (median3-of-median3) of those
/// indexes into index `e` (aka 5, the middle index).
#[allow(clippy::too_many_arguments)] // Keep the nine sample positions explicit.
pub fn ninther<T: Ord>(
    r: &mut [T],
    a: usize,
    b: usize,
    c: usize,
    mut d: usize,
    e: usize,
    mut f: usize,
    g: usize,
    h: usize,
    i: usize,
) {
    let mut m1 = median3_index(r, a, b, c);
    let mut m3 = median3_index(r, g, h, i);
    if r[m1] > r[m3] {
        core::mem::swap(&mut m1, &mut m3);
    }

    if r[d] > r[f] {
        core::mem::swap(&mut d, &mut f);
    }
    // Here we know that r[_2] and r[_8] are the other two medians and that
    // r[_2] <= r[_8]. We also know that r[_4] <= r[_6]
    if r[e] < r[d] {
        // r[_4] is the median of r[_4], r[_5], r[_6]
    } else if r[e] > r[f] {
        // r[_6] is the median of r[_4], r[_5], r[_6]
        d = f;
    } else {
        // Here we know r[_5] is the median of r[_4], r[_5], r[_6]
        if r[e] < r[m1] {
            instrumentation::swap(r, e, m1);
            return;
        }
        if r[e] > r[m3] {
            instrumentation::swap(r, e, m3);
            return;
        }
        // This is the only path that returns with no swap
        return;
    }
    // Here we know r[_4] is the median of r[_4], r[_5], r[_6]
    if r[d] < r[m1] {
        d = m1;
    } else if r[d] > r[m3] {
        d = m3;
    }
    instrumentation::swap(r, e, d);
}

/*
Data: A
Result: Pivot p, 0 ≤ p < |A|; A partitioned at p
1 if |A| < 9 then
2 return HoarePartition(A, |A|/2);
3 end
4 f ← |A|/9;
5 for i ← 4f through 5f − 1 do
6 l ← i − 4f ;
7 r ← i + 5f ;
8 Ninther(A, l, l + 1, l + 2, l + 3, i,
r, r + 1, r + 2, r + 3);
9 end
10 Quickselect(RepeatedStepBasic,
A[4f : 5f ], f /2);
11 return
ExpandPartition(A, 4f, 4f + f /2, 5f − 1);
 */
/*
pub fn median_of_ninthers_basic<T: Ord>(a: &mut [T])
    -> usize
{
    let l = a.len();
    if l < 9 {
        return hoare_partition(a, l/2);
    }

    let f = l / 9;

    for i in (4*f)..(5*f) {
        let l = i - 4*f;
        let r = i + 5*f;
        ninther(a, l, l+1, l+2, l+3, i,
                r, r+1, r+2, r+3);
    }

    quickselect(median_of_ninthers_basic, &mut a[4*f..5*f], f/2);
    expand_parition(a, 4*f, 4*f+f/2, 5*f-1);
}
    */

/// median-of-medians on groups of 3 elements
pub fn repeated_step3<T: Ord>(a: &mut [T]) -> usize {
    let l = a.len();
    if l < 9 {
        return hoare_partition(a, l / 2);
    }

    let mut i = 0;
    let mut j = 0;

    while i + 2 < a.len() {
        instrumentation::scanned(3);
        sort3((&mut a[i..i + 3]).try_into().unwrap());
        instrumentation::swap(a, i + 1, j);
        i += 3;
        j += 1;
    }

    let mut i = 0;
    let mut m = 0;
    while i + 2 < j {
        instrumentation::scanned(3);
        sort3((&mut a[i..i + 3]).try_into().unwrap());
        instrumentation::swap(a, i + 1, m);
        i += 3;
        m += 1;
    }

    quickselect(repeated_step3, &mut a[..m], m / 2);
    hoare_partition(a, m / 2)
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
pub fn bfprt_baseline<T: Ord>(a: &mut [T]) -> usize {
    let l = a.len();
    if l < 5 {
        return hoare_partition(a, l / 2);
    }

    let mut i = 0;
    let mut j = 0;

    while i + 4 < a.len() {
        instrumentation::scanned(5);
        partition5((&mut a[i..i + 5]).try_into().unwrap());
        instrumentation::swap(a, i + 2, j);
        i += 5;
        j += 1;
    }

    quickselect(bfprt_baseline, &mut a[0..j], j / 2);
    hoare_partition(a, j / 2)
}

/*
Data: A
Result: Pivot 0 ≤ p < |A|; A partitioned at A[p]
1 if |A| < 9 then
2 return HoarePartition(A, |A|/2);
3 end
4 i ← 0; j ← 0;
5 while i + 2 < |A| do
6 Median3(A, i, i + 1, i + 2);
7 Swap(A[i + 1], A[j]);
8 i ← i + 3;
9 j ← j + 1;
10 end
11 i ← 0; m ← 0;
12 while i + 2 < j do
13 Median3(A, i, i + 1, i + 2);
14 Swap(A[i + 1], A[m]);
15 i ← i + 3;
16 m ← m + 1;
17 end
18 Quickselect(RepeatedStep, A[0 : m], m/2);
19 return HoarePartition(A, m/2);
 */
pub fn repeated_step<T: Ord>(a: &mut [T]) -> usize {
    if a.len() < 9 {
        return hoare_partition(a, a.len() / 2);
    }

    let mut i = 0;
    let mut j = 0;

    while i + 2 < a.len() {
        instrumentation::scanned(3);
        sort3((&mut a[i..(i + 3)]).try_into().unwrap());
        instrumentation::swap(a, i + 1, j);
        i += 3;
        j += 1;
    }

    let mut i = 0;
    let mut m = 0;
    while i + 2 < j {
        instrumentation::scanned(3);
        sort3((&mut a[i..(i + 3)]).try_into().unwrap());
        instrumentation::swap(a, i + 1, m);
        i += 3;
        m += 1;
    }

    quickselect(repeated_step, &mut a[0..m], m / 2);
    hoare_partition(a, m / 2)
}

/*
fn bfprt_improved<T: Ord>(a: &mut [T])
{
    let l = a.len();
    if l < 5 {
        return hoare_partition(a, l/2);
    }

    let f = l/5;
    let mut i = 0;
    for j in 2*f..3*f {
        median5(a,i,i+1,j,3*f+i,3*f+i+1);
        i += 2;
    }

    quickselect(bfprt_improved, a[2*f..3*f], f/2);
    expand_partition(a, 2*f, 2*f+f/2,3*f-1);
}
*/

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
pub fn hoare_partition<T: Ord>(arr: &mut [T], pivot: usize) -> usize {
    instrumentation::partition_pass();
    let p = pivot;
    debug_assert!(!arr.is_empty());
    debug_assert!(p < arr.len());

    instrumentation::swap(arr, 0, p);
    let mut a = 1;
    let mut b = arr.len() - 1;

    'a: loop {
        loop {
            if a > b {
                break 'a;
            }

            instrumentation::scanned(1);
            if arr[a] >= arr[0] {
                break;
            }

            a += 1;
        }

        while {
            instrumentation::scanned(1);
            arr[0] < arr[b]
        } {
            b -= 1;
        }

        if a >= b {
            break;
        }

        instrumentation::swap(arr, a, b);
        a += 1;
        b -= 1;
    }

    a -= 1;
    instrumentation::swap(arr, 0, a);
    a
}

#[cfg(test)]
mod test;
