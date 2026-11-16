use arsort::prelude::*;
use std::mem::MaybeUninit;

#[cfg(not(feature = "large-indices"))]
pub type IndexType = u32;
#[cfg(not(feature = "large-indices"))]
pub type GenerationType = std::num::NonZeroU32;

#[cfg(feature = "large-indices")]
pub type IndexType = u64;
#[cfg(feature = "large-indices")]
pub type GenerationType = std::num::NonZeroU64;

/// Sweet spot threshold of `arsort` (L1/L2 cache-bound domain).
/// Datasets larger than this threshold undergo a single-pass disjoint scatter
/// into cache-sized buckets before invoking `arsort`.
const MACRO_PARTITION_THRESHOLD: usize = 20000;

// =====================================================================
// 1. Data Structures (Generational Management)
// =====================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Index {
    pub index: IndexType,
    pub generation: GenerationType,
}

impl SortKey for Index {
    #[inline(always)]
    fn sort_key(&self) -> u64 {
        self.index as u64
    }
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
    pub(crate) data: Vec<Entry<T>>,
    pub(crate) free_head: Option<IndexType>,
}

// =====================================================================
// 2. Inner Workspace (v0.3.0 Single-Pass Scatter + Arsort Architecture)
// =====================================================================

struct Workspace {
    buffer: Vec<MaybeUninit<Index>>,
    counts: Vec<usize>,
    offsets: Vec<usize>,
}

impl Workspace {
    fn new() -> Self {
        Self {
            buffer: Vec::new(),
            counts: Vec::new(),
            offsets: Vec::new(),
        }
    }

    #[inline(always)]
    fn prepare(&mut self, n: usize, num_buckets: usize) {
        if self.buffer.capacity() < n {
            self.buffer.reserve(n - self.buffer.len());
        }
        unsafe {
            self.buffer.set_len(n);
        }
        self.counts.clear();
        self.counts.resize(num_buckets, 0);
        self.offsets.clear();
        self.offsets.resize(num_buckets, 0);
    }
}

// =====================================================================
// 3. Dynamic Expansion Components (VerifiedIndices)
// =====================================================================

pub struct VerifiedIndices {
    pub(crate) ordered: Vec<Index>,
    ws: Workspace,
}

impl VerifiedIndices {
    pub fn new(mut indices: Vec<Index>) -> Self {
        let mut ws = Workspace::new();
        Self::build_from_indices(&mut indices, &mut ws);
        Self {
            ordered: indices,
            ws,
        }
    }

    fn build_from_indices(indices: &mut [Index], ws: &mut Workspace) {
        let n = indices.len();
        if n <= 1 {
            return;
        }

        if n <= MACRO_PARTITION_THRESHOLD {
            arsort(indices);
        } else {
            Self::partition_and_sort_macro(indices, ws);
        }

        for i in 0..(n - 1) {
            if indices[i].index == indices[i + 1].index {
                panic!(
                    "Aliasing detected: multiple mutable references requested for index {}.",
                    indices[i].index
                );
            }
        }
    }

    fn partition_and_sort_macro(indices: &mut [Index], ws: &mut Workspace) {
        let n = indices.len();

        let mut min_key = u64::MAX;
        let mut max_key = u64::MIN;
        for idx in indices.iter() {
            let k = idx.index as u64;
            if k < min_key {
                min_key = k;
            }
            if k > max_key {
                max_key = k;
            }
        }

        if min_key == max_key {
            return;
        }

        let num_buckets = (n / 2048).next_power_of_two().clamp(16, 16384);
        let bucket_mask = num_buckets - 1;

        let range = max_key - min_key;
        let multiplier: u128 = ((num_buckets as u128) << 64) / ((range as u128) + 1);

        ws.prepare(n, num_buckets);

        // 1. Single-pass histogram
        for idx in indices.iter() {
            let diff = (idx.index as u64 - min_key) as u128;
            let bucket = (((diff * multiplier) >> 64) as usize) & bucket_mask;
            ws.counts[bucket] += 1;
        }

        // 2. Prefix sum
        let mut sum = 0;
        for (offset, &count) in ws.offsets.iter_mut().zip(&ws.counts) {
            *offset = sum;
            sum += count;
        }

        // 3. Disjoint single-pass scatter
        let buf_ptr = ws.buffer.as_mut_ptr();
        for &idx in indices.iter() {
            let diff = (idx.index as u64 - min_key) as u128;
            let bucket = (((diff * multiplier) >> 64) as usize) & bucket_mask;
            let pos = ws.offsets[bucket];
            unsafe {
                buf_ptr.add(pos).write(MaybeUninit::new(idx));
            }
            ws.offsets[bucket] = pos + 1;
        }

        // 4. Local `arsort` per bucket and sequential write-back
        let mut start = 0;
        for &count in &ws.counts {
            if count == 0 {
                continue;
            }
            let end = start + count;

            let bucket_slice: &mut [Index] =
                unsafe { std::slice::from_raw_parts_mut(buf_ptr.add(start) as *mut Index, count) };

            if count > 1 {
                arsort(bucket_slice);
            }

            indices[start..end].copy_from_slice(bucket_slice);
            start = end;
        }
    }

    pub fn clear_and_verify(&mut self, new_targets: &[Index]) {
        self.ordered.clear();

        if new_targets.is_empty() {
            return;
        }

        self.ordered.extend_from_slice(new_targets);
        Self::build_from_indices(&mut self.ordered, &mut self.ws);
    }

    pub fn as_slice(&self) -> &[Index] {
        &self.ordered
    }
}

// =====================================================================
// 4. Core OrdexArena Implementation & OrdexIter
// =====================================================================

pub struct OrdexIter<'a, T> {
    arena_ptr: *mut Entry<T>,
    ordered: &'a [Index],
    pos: usize,
}

impl<'a, T: 'a> Iterator for OrdexIter<'a, T> {
    type Item = &'a mut T;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.ordered.len() {
            return None;
        }
        let idx = self.ordered[self.pos];
        self.pos += 1;

        let i = usize::try_from(idx.index).unwrap();
        unsafe {
            match &mut *self.arena_ptr.add(i) {
                Entry::Occupied { value, .. } => Some(value),
                Entry::Free { .. } => core::hint::unreachable_unchecked(),
            }
        }
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.ordered.len() - self.pos;
        (remaining, Some(remaining))
    }
}

impl<'a, T: 'a> ExactSizeIterator for OrdexIter<'a, T> {}

impl<T> Default for OrdexArena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> OrdexArena<T> {
    pub(crate) const START_GENERATION: GenerationType = GenerationType::new(1).unwrap();

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

    #[inline(always)]
    pub fn ordex<F, R>(&mut self, verified: &VerifiedIndices, f: F) -> R
    where
        F: FnOnce(OrdexIter<'_, T>) -> R,
    {
        for idx in &verified.ordered {
            let i = usize::try_from(idx.index)
                .expect("Index is too large for the target architecture memory size");
            match self.data.get(i) {
                Some(Entry::Occupied { generation, .. }) => {
                    assert_eq!(
                        *generation, idx.generation,
                        "Generation mismatch at index {}: element has been removed or replaced.",
                        i
                    );
                }
                Some(Entry::Free { .. }) => {
                    panic!("Dangling pointer: Entry at index {} is already freed.", i)
                }
                None => panic!("Index out of bounds: index {} is out of arena capacity.", i),
            }
        }

        let iter = OrdexIter {
            arena_ptr: self.data.as_mut_ptr(),
            ordered: &verified.ordered,
            pos: 0,
        };
        f(iter)
    }
}
