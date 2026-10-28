// Expand each adapter at the call site so tiny inputs do not measure indirect calls.
macro_rules! for_each {
    ($register:ident, $values:ident, $index:ident) => {
        $register!("kth", $values, $index, {
            kth::partition_by_kth($values, $index);
        });
        $register!("std", $values, $index, {
            $values.select_nth_unstable($index);
        });
        $register!("order-stat", $values, $index, {
            order_stat::kth($values, $index);
        });
        $register!("pdqselect", $values, $index, {
            pdqselect::select($values, $index);
        });
        $register!("adqselect", $values, $index, {
            adqselect::nth_element($values, $index, &mut Ord::cmp);
        });
        $register!("tpt-simd-sort", $values, $index, {
            tpt_simd_sort::select_nth($values, $index);
        });
        $register!("turboselect", $values, $index, {
            turboselect::select_nth_unstable($values, $index);
        });
        $register!("sort", $values, $index, {
            $values.sort();
        });
        $register!("sort_unstable", $values, $index, {
            $values.sort_unstable();
        });
    };
}

pub(crate) use for_each;
