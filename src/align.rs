use crate::core::{Entry, Index, OrdexArena};

macro_rules! impl_align {
    ($fn_name:ident $(, $idx:ident : $t:ty)+) => {
        #[inline(always)]
        pub fn $fn_name<F, R>(&mut self $(, $idx: Index)+, f: F) -> R
        where
            F: FnOnce($(&mut $t),+) -> R,
        {
            let __ordex_len = self.data.len();

            $(
                let __ordex_idx_usize = usize::try_from($idx.index)
                    .expect("Index is too large for the target architecture memory size");
                assert!(
                    __ordex_idx_usize < __ordex_len,
                    "Index out of bounds: index {} is out of arena capacity. If its existence is uncertain, use `get()` or `get_mut()` instead of `align!`.",
                    __ordex_idx_usize
                );
                match &self.data[__ordex_idx_usize] {
                    Entry::Occupied { generation, .. } => {
                        assert_eq!(
                            *generation,
                            $idx.generation,
                            "Generation mismatch at index {}: This element has already been removed or replaced. If its existence is uncertain, use `get()` or `get_mut()` instead of `align!`.",
                            __ordex_idx_usize
                        );
                    }
                    Entry::Free { .. } => panic!(
                        "Dangling pointer: Entry at index {} is already freed. If its existence is uncertain, use `get()` or `get_mut()` instead of `align!`.",
                        __ordex_idx_usize
                    ),
                }
            )+

            let __ordex_indices = [ $($idx.index),+ ];
            let __ordex_len = __ordex_indices.len();
            for i in 0..__ordex_len {
                for j in (i + 1)..__ordex_len {
                    assert!(
                        __ordex_indices[i] != __ordex_indices[j],
                        "Aliasing detected: Multiple mutable references requested for the same index."
                    );
                }
            }

            unsafe {
                let __ordex_ptr = self.data.as_mut_ptr();
                f($(
                    match &mut *__ordex_ptr.add(usize::try_from($idx.index).unwrap_unchecked()) {
                        Entry::Occupied { value, .. } => value,
                        _ => core::hint::unreachable_unchecked(),
                    }
                ),+)
            }
        }
    };
}

#[macro_export]
macro_rules! align {
    ($arena:expr, $i0:expr, $i1:expr, $closure:expr) => {
        $arena.align2($i0, $i1, $closure)
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $closure:expr) => {
        $arena.align3($i0, $i1, $i2, $closure)
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $closure:expr) => {
        $arena.align4($i0, $i1, $i2, $i3, $closure)
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $i4:expr, $closure:expr) => {
        $arena.align5($i0, $i1, $i2, $i3, $i4, $closure)
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $i4:expr, $i5:expr, $closure:expr) => {
        $arena.align6($i0, $i1, $i2, $i3, $i4, $i5, $closure)
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $i4:expr, $i5:expr, $i6:expr, $closure:expr) => {
        $arena.align7($i0, $i1, $i2, $i3, $i4, $i5, $i6, $closure)
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $i4:expr, $i5:expr, $i6:expr, $i7:expr, $closure:expr) => {
        $arena.align8($i0, $i1, $i2, $i3, $i4, $i5, $i6, $i7, $closure)
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $i4:expr, $i5:expr, $i6:expr, $i7:expr, $i8:expr, $closure:expr) => {
        $arena.align9($i0, $i1, $i2, $i3, $i4, $i5, $i6, $i7, $i8, $closure)
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $i4:expr, $i5:expr, $i6:expr, $i7:expr, $i8:expr, $i9:expr, $closure:expr) => {
        $arena.align10($i0, $i1, $i2, $i3, $i4, $i5, $i6, $i7, $i8, $i9, $closure)
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $i4:expr, $i5:expr, $i6:expr, $i7:expr, $i8:expr, $i9:expr, $i10:expr, $closure:expr) => {
        $arena.align11(
            $i0, $i1, $i2, $i3, $i4, $i5, $i6, $i7, $i8, $i9, $i10, $closure,
        )
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $i4:expr, $i5:expr, $i6:expr, $i7:expr, $i8:expr, $i9:expr, $i10:expr, $i11:expr, $closure:expr) => {
        $arena.align12(
            $i0, $i1, $i2, $i3, $i4, $i5, $i6, $i7, $i8, $i9, $i10, $i11, $closure,
        )
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $i4:expr, $i5:expr, $i6:expr, $i7:expr, $i8:expr, $i9:expr, $i10:expr, $i11:expr, $i12:expr, $closure:expr) => {
        $arena.align13(
            $i0, $i1, $i2, $i3, $i4, $i5, $i6, $i7, $i8, $i9, $i10, $i11, $i12, $closure,
        )
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $i4:expr, $i5:expr, $i6:expr, $i7:expr, $i8:expr, $i9:expr, $i10:expr, $i11:expr, $i12:expr, $i13:expr, $closure:expr) => {
        $arena.align14(
            $i0, $i1, $i2, $i3, $i4, $i5, $i6, $i7, $i8, $i9, $i10, $i11, $i12, $i13, $closure,
        )
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $i4:expr, $i5:expr, $i6:expr, $i7:expr, $i8:expr, $i9:expr, $i10:expr, $i11:expr, $i12:expr, $i13:expr, $i14:expr, $closure:expr) => {
        $arena.align15(
            $i0, $i1, $i2, $i3, $i4, $i5, $i6, $i7, $i8, $i9, $i10, $i11, $i12, $i13, $i14, $closure,
        )
    };
    ($arena:expr, $i0:expr, $i1:expr, $i2:expr, $i3:expr, $i4:expr, $i5:expr, $i6:expr, $i7:expr, $i8:expr, $i9:expr, $i10:expr, $i11:expr, $i12:expr, $i13:expr, $i14:expr, $i15:expr, $closure:expr) => {
        $arena.align16(
            $i0, $i1, $i2, $i3, $i4, $i5, $i6, $i7, $i8, $i9, $i10, $i11, $i12, $i13, $i14, $i15, $closure,
        )
    };
    ($arena:expr, $($idx:expr),+ $(,)?) => {
        compile_error!("align! macro supports up to 16 indices for optimal register and I-cache usage. For 17 or more elements, use OrdexArena::ordex() to maintain performance and order.");
    };
}

impl<T> OrdexArena<T> {
    impl_align!(align2, i0: T, i1: T);
    impl_align!(align3, i0: T, i1: T, i2: T);
    impl_align!(align4, i0: T, i1: T, i2: T, i3: T);
    impl_align!(align5, i0: T, i1: T, i2: T, i3: T, i4: T);
    impl_align!(align6, i0: T, i1: T, i2: T, i3: T, i4: T, i5: T);
    impl_align!(align7, i0: T, i1: T, i2: T, i3: T, i4: T, i5: T, i6: T);
    impl_align!(align8, i0: T, i1: T, i2: T, i3: T, i4: T, i5: T, i6: T, i7: T);
    impl_align!(align9, i0: T, i1: T, i2: T, i3: T, i4: T, i5: T, i6: T, i7: T, i8: T);
    impl_align!(align10, i0: T, i1: T, i2: T, i3: T, i4: T, i5: T, i6: T, i7: T, i8: T, i9: T);
    impl_align!(align11, i0: T, i1: T, i2: T, i3: T, i4: T, i5: T, i6: T, i7: T, i8: T, i9: T, i10: T);
    impl_align!(align12, i0: T, i1: T, i2: T, i3: T, i4: T, i5: T, i6: T, i7: T, i8: T, i9: T, i10: T, i11: T);
    impl_align!(align13, i0: T, i1: T, i2: T, i3: T, i4: T, i5: T, i6: T, i7: T, i8: T, i9: T, i10: T, i11: T, i12: T);
    impl_align!(align14, i0: T, i1: T, i2: T, i3: T, i4: T, i5: T, i6: T, i7: T, i8: T, i9: T, i10: T, i11: T, i12: T, i13: T);
    impl_align!(align15, i0: T, i1: T, i2: T, i3: T, i4: T, i5: T, i6: T, i7: T, i8: T, i9: T, i10: T, i11: T, i12: T, i13: T, i14: T);
    impl_align!(align16, i0: T, i1: T, i2: T, i3: T, i4: T, i5: T, i6: T, i7: T, i8: T, i9: T, i10: T, i11: T, i12: T, i13: T, i14: T, i15: T);
}
