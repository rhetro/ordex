// src/core.rs

use std::marker::PhantomData;

#[cfg(not(feature = "large-indices"))]
pub type IndexType = u32;
#[cfg(not(feature = "large-indices"))]
pub type GenerationType = std::num::NonZeroU32;

#[cfg(feature = "large-indices")]
pub type IndexType = u64;
#[cfg(feature = "large-indices")]
pub type GenerationType = std::num::NonZeroU64;

// =====================================================================
// 1. Data Structures (Generational Management)
// =====================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Index {
    pub index: IndexType,
    pub generation: GenerationType,
}

pub enum Entry<T> {
    Free {
        next_free: Option<IndexType>,
        generation: GenerationType,
    },
    Occupied {
        generation: GenerationType,
        value: T,
    },
}

pub struct OrdexArena<T> {
    data: Vec<Entry<T>>,
    free_head: Option<IndexType>,
}

// =====================================================================
// 2. Macros (Static Expansion & Ergonomics)
// =====================================================================

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

// =====================================================================
// 3. Dynamic Expansion Components
// =====================================================================

pub struct VerifiedIndices {
    indices: Vec<Index>,
}

impl VerifiedIndices {
    pub fn new(mut raw_indices: Vec<Index>) -> Self {
        raw_indices.sort_unstable_by_key(|idx| idx.index);
        for i in 1..raw_indices.len() {
            assert!(
                raw_indices[i - 1].index != raw_indices[i].index,
                "Aliasing detected: Multiple mutable references to the same physical index"
            );
        }
        Self {
            indices: raw_indices,
        }
    }

    pub fn clear_and_verify(&mut self, new_targets: &[Index]) {
        self.indices.clear();
        self.indices.extend_from_slice(new_targets);
        self.indices.sort_unstable_by_key(|idx| idx.index);
        for i in 1..self.indices.len() {
            assert!(
                self.indices[i - 1].index != self.indices[i].index,
                "Aliasing detected: Multiple mutable references to the same physical index"
            );
        }
    }

    pub fn as_slice(&self) -> &[Index] {
        &self.indices
    }
}

pub struct OrdexArenaIter<'a, T> {
    ptr: *mut Entry<T>,
    indices: std::slice::Iter<'a, Index>,
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T> Iterator for OrdexArenaIter<'a, T> {
    type Item = &'a mut T;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.indices.next().map(|idx| unsafe {
            let idx_usize = usize::try_from(idx.index).unwrap_unchecked();
            match &mut *self.ptr.add(idx_usize) {
                Entry::Occupied { value, .. } => value,
                _ => core::hint::unreachable_unchecked(),
            }
        })
    }
}

// =====================================================================
// 4. Core OrdexArena Implementation
// =====================================================================

impl<T> OrdexArena<T> {
    const START_GENERATION: GenerationType = unsafe { GenerationType::new_unchecked(1) };

    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            free_head: None,
        }
    }

    pub fn insert(&mut self, value: T) -> Index {
        match self.free_head {
            Some(free_idx) => {
                let idx = usize::try_from(free_idx)
                    .expect("Index is too large for the target architecture memory size");
                match self.data[idx] {
                    Entry::Free {
                        next_free,
                        generation,
                    } => {
                        self.free_head = next_free;
                        let next_val = generation.get().wrapping_add(1);
                        let next_gen = if next_val == 0 {
                            Self::START_GENERATION
                        } else {
                            GenerationType::new(next_val).unwrap()
                        };

                        self.data[idx] = Entry::Occupied {
                            generation: next_gen,
                            value,
                        };
                        Index {
                            index: free_idx,
                            generation: next_gen,
                        }
                    }
                    _ => unreachable!("Free list is corrupted"),
                }
            }
            None => {
                let idx = IndexType::try_from(self.data.len())
                    .expect("OrdexArena capacity exceeded: Maximum simultaneous elements reached");
                let start_gen = Self::START_GENERATION;

                self.data.push(Entry::Occupied {
                    generation: start_gen,
                    value,
                });
                Index {
                    index: idx,
                    generation: start_gen,
                }
            }
        }
    }

    #[inline(always)]
    fn next_generation(current: GenerationType) -> GenerationType {
        let next_val = current.get().wrapping_add(1);
        GenerationType::new(if next_val == 0 { 1 } else { next_val }).unwrap()
    }

    pub fn remove(&mut self, index: Index) -> Option<T> {
        let idx_usize = usize::try_from(index.index).ok()?;
        if let Some(Entry::Occupied { generation, .. }) = self.data.get(idx_usize) {
            if *generation == index.generation {
                let next_gen = Self::next_generation(*generation);

                let old_entry = std::mem::replace(
                    &mut self.data[idx_usize],
                    Entry::Free {
                        next_free: self.free_head,
                        generation: next_gen,
                    },
                );
                self.free_head = Some(index.index);

                if let Entry::Occupied { value, .. } = old_entry {
                    return Some(value);
                }
            }
        }
        None
    }

    // ---------------------------------------------------------------------
    // Full Iteration
    // ---------------------------------------------------------------------

    pub fn iter(&self) -> impl Iterator<Item = (Index, &T)> {
        self.data
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| match entry {
                Entry::Occupied { generation, value } => Some((
                    Index {
                        index: i as IndexType,
                        generation: *generation,
                    },
                    value,
                )),
                Entry::Free { .. } => None,
            })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Index, &mut T)> {
        self.data
            .iter_mut()
            .enumerate()
            .filter_map(|(i, entry)| match entry {
                Entry::Occupied { generation, value } => Some((
                    Index {
                        index: i as IndexType,
                        generation: *generation,
                    },
                    value,
                )),
                Entry::Free { .. } => None,
            })
    }

    // ---------------------------------------------------------------------
    // Safe Access (Non-panicking)
    // ---------------------------------------------------------------------

    pub fn get(&self, idx: Index) -> Option<&T> {
        let idx_usize = usize::try_from(idx.index).ok()?;
        self.data.get(idx_usize).and_then(|entry| match entry {
            Entry::Occupied { generation, value } if *generation == idx.generation => Some(value),
            _ => None,
        })
    }

    pub fn get_mut(&mut self, idx: Index) -> Option<&mut T> {
        let idx_usize = usize::try_from(idx.index).ok()?;
        self.data.get_mut(idx_usize).and_then(|entry| match entry {
            Entry::Occupied { generation, value } if *generation == idx.generation => Some(value),
            _ => None,
        })
    }

    // ---------------------------------------------------------------------
    // Static Expansion Methods (align, up to n=16)
    // ---------------------------------------------------------------------
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

    // ---------------------------------------------------------------------
    // Dynamic Expansion Method (ordex)
    // ---------------------------------------------------------------------
    #[inline(always)]
    pub fn ordex<F, R>(&mut self, verified: &VerifiedIndices, f: F) -> Option<R>
    where
        F: for<'a> FnOnce(OrdexArenaIter<'a, T>) -> R,
    {
        if verified.indices.is_empty() {
            return None;
        }

        if let Some(max_idx) = verified.indices.last() {
            let max_usize = usize::try_from(max_idx.index)
                .expect("Index is too large for the target architecture memory size");
            assert!(
                max_usize < self.data.len(),
                "Index out of bounds: index {} is out of arena capacity. If its existence is uncertain, use `get()` or `get_mut()` instead of `ordex`.",
                max_usize
            );
        }

        for idx in &verified.indices {
            let idx_usize = usize::try_from(idx.index).unwrap();
            match &self.data[idx_usize] {
                Entry::Occupied { generation, .. } => {
                    assert_eq!(
                        *generation, idx.generation,
                        "Generation mismatch at index {}: This element has already been removed or replaced. If its existence is uncertain, use `get()` or `get_mut()` instead of `ordex`.",
                        idx_usize
                    );
                }
                Entry::Free { .. } => panic!(
                    "Dangling pointer: Entry at index {} is already freed. If its existence is uncertain, use `get()` or `get_mut()` instead of `ordex`.",
                    idx_usize
                ),
            }
        }

        let iter = OrdexArenaIter {
            ptr: self.data.as_mut_ptr(),
            indices: verified.indices.iter(),
            _marker: PhantomData,
        };

        Some(f(iter))
    }
}
