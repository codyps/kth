#![cfg_attr(not(test), no_std)]

#[cfg(test)]
#[macro_use]
extern crate quickcheck;

#[macro_use]
extern crate index_fixed;

pub mod quickselect;

pub trait SliceExtKth {
    /// Re-order the slice so that the element with the order given by pivot order (ie: the element
    /// at the k-th index when the array is sorted) has all elements smaller than it before it, and
    /// all elements larger than it afterwards.
    ///
    /// The k-th element can then be read from `self[pivot_order]`, if desired.
    ///
    /// # Panics
    ///
    ///  - If the slice has length zero.
    ///  - If the pivot_order is larger than the slice length.
    ///
    /// # Examples
    ///
    /// ```
    /// use kth::SliceExtKth;
    /// //          [2,2,3,4,9];
    /// let mut x = [3,9,2,2,4];
    /// let m_loc = x.len()/2;
    /// x.partition_by_kth(m_loc);
    /// let median = x[m_loc];
    /// assert_eq!(median, 3);
    /// ```
    /// 
    fn partition_by_kth(&mut self, pivot_order: usize);
}

impl<T: Ord> SliceExtKth for [T] {
    fn partition_by_kth(&mut self, pivot_order: usize)
    {
        quickselect::quickselect(quickselect::repeated_step3, self, pivot_order)
    }
}
