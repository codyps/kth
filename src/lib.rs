//! Algorithms to find the K-th order element
//!
//! The k-th order element is the element which would be at the k-th index if the array was sorted.
//!
//! Finding the median is a special case of finding the k-th order element, to find the median,
//! select k to be half the array length.
//!
//! Partitioning is commonly performed when searching for the K-th order element. An array is
//! partitioned if all elements before a given element X are less than X, and all elements after a
//! that same element X are greater than X.
//!
//!
//! # Example
//! ```
//! use kth::SliceExtKth;
//!
//! let mut x = [6, 6, 8 ,1, 2];
//! // sorted =  1  2  6  6  8
//! let m = x.len()/2;
//! x.partition_by_kth(m);
//! println!("Median is {}", x[m]);
//! assert_eq!(x[x.len()/2], 6);
//! ```

#![cfg_attr(not(test), no_std)]
#![cfg_attr(all(test, feature = "nightly"), feature(test))]

#[cfg(test)]
#[macro_use]
extern crate quickcheck;

#[macro_use]
extern crate index_fixed;

mod quickselect;

/// Add k-th order element operations to slices.
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
