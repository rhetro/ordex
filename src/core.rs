use std::mem::MaybeUninit;

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
    pub(crate) data: Vec<Entry<T>>,
    pub(crate) free_head: Option<IndexType>,
}

// =====================================================================
// 2. Inner Workspace (v0.2.0 Mid SoA Architecture)
// =====================================================================

#[repr(C, align(64))]
struct ChunkData {
    indices: [MaybeUninit<Index>; 16],
}

impl Default for ChunkData {
    #[inline(always)]
    fn default() -> Self {
        Self {
            indices: [const { MaybeUninit::uninit() }; 16],
        }
    }
}

#[derive(Clone, Copy, Default)]
struct ChunkMeta {
    bitmap: u16,
    occupancy: u8,
    is_dirty: bool,
}

struct Workspace {
    metas: Vec<ChunkMeta>,
    chunks: Vec<ChunkData>,
    overflow: Vec<(usize, MaybeUninit<Index>)>,
}

impl Workspace {
    fn new() -> Self {
        Self {
            metas: Vec::new(),
            chunks: Vec::new(),
            overflow: Vec::new(),
        }
    }

    #[inline(always)]
    fn prepare(&mut self, c: usize) {
        self.metas.clear();
        self.metas.resize(c, ChunkMeta::default());
        self.chunks.clear();
        self.chunks.resize_with(c, ChunkData::default);
        self.overflow.clear();
    }
}

#[inline(always)]
fn sort_overflow(overflow: &mut [(usize, MaybeUninit<Index>)]) {
    let len = overflow.len();
    if len <= 1 {
        return;
    }

    for i in 1..len {
        let mut j = i;
        while j > 0 {
            let key_a = overflow[j - 1].0;
            let key_b = overflow[j].0;
            let idx_a = unsafe { overflow[j - 1].1.assume_init().index };
            let idx_b = unsafe { overflow[j].1.assume_init().index };

            if key_a > key_b || (key_a == key_b && idx_a > idx_b) {
                overflow.swap(j - 1, j);
                j -= 1;
            } else {
                break;
            }
        }
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
    pub fn new(indices: Vec<Index>) -> Self {
        let n = indices.len();
        if n == 0 {
            return Self {
                ordered: Vec::new(),
                ws: Workspace::new(),
            };
        }

        let min_key = indices.iter().map(|idx| idx.index as u64).min().unwrap();
        let max_key = indices.iter().map(|idx| idx.index as u64).max().unwrap();
        let range = max_key.saturating_sub(min_key);

        let c = std::cmp::max(1, n / 4);
        let m = (c * 16 - 1) as u64;
        let multiplier = if range == 0 {
            0
        } else {
            (((m as u128) << 32) / (range as u128)) as u64
        };

        let mut ws = Workspace::new();
        ws.prepare(c);

        let ordered = Self::build_from_indices(&indices, c, min_key, multiplier, &mut ws);

        Self { ordered, ws }
    }

    fn build_from_indices(
        indices: &[Index],
        c: usize,
        min_key: u64,
        multiplier: u64,
        ws: &mut Workspace,
    ) -> Vec<Index> {
        let n = indices.len();
        let metas = &mut ws.metas;
        let chunks = &mut ws.chunks;
        let overflow = &mut ws.overflow;

        for &idx in indices {
            let key = idx.index as u64;
            let diff = key - min_key;
            let i_v = ((diff as u128 * multiplier as u128) >> 32) as usize;

            let chunk_id = std::cmp::min(i_v >> 4, c - 1);
            let offset = i_v & 15;

            let meta = &mut metas[chunk_id];
            let data_chunk = &mut chunks[chunk_id];

            let bit = 1 << offset;
            if (meta.bitmap & bit) != 0 {
                let existing = unsafe { data_chunk.indices[offset].assume_init() };

                if existing.index == idx.index && existing.generation == idx.generation {
                    panic!(
                        "Aliasing detected: multiple mutable references requested for index {}.",
                        idx.index
                    );
                } else {
                    meta.is_dirty = true;
                    let empty_offset = (!meta.bitmap).trailing_zeros() as usize;

                    if empty_offset < 16 {
                        data_chunk.indices[empty_offset].write(idx);
                        meta.bitmap |= 1 << empty_offset;
                        meta.occupancy += 1;
                    } else {
                        overflow.push((chunk_id, MaybeUninit::new(idx)));
                        meta.occupancy += 1;
                    }
                }
            } else {
                data_chunk.indices[offset].write(idx);
                meta.bitmap |= bit;
                meta.occupancy += 1;
            }
        }

        if overflow.len() > 1 {
            sort_overflow(overflow);
        }

        let mut ordered = Vec::with_capacity(n);
        let mut overflow_idx = 0;

        for id in 0..c {
            let meta = metas[id];
            if meta.occupancy == 0
                && (overflow_idx >= overflow.len() || overflow[overflow_idx].0 != id)
            {
                continue;
            }

            let data_chunk = &chunks[id];
            let mut bmp = meta.bitmap;

            while bmp != 0 {
                let offset = bmp.trailing_zeros() as usize;
                let idx = unsafe { data_chunk.indices[offset].assume_init() };
                ordered.push(idx);
                bmp &= !(1 << offset);
            }

            while overflow_idx < overflow.len() && overflow[overflow_idx].0 == id {
                let idx = unsafe { overflow[overflow_idx].1.assume_init() };
                ordered.push(idx);
                overflow_idx += 1;
            }
        }

        ordered
    }

    pub fn clear_and_verify(&mut self, new_targets: &[Index]) {
        self.ordered.clear();

        if new_targets.is_empty() {
            return;
        }

        let n = new_targets.len();
        if n == 1 {
            self.ordered.extend_from_slice(new_targets);
            return;
        }

        let min_key = new_targets
            .iter()
            .map(|idx| idx.index as u64)
            .min()
            .unwrap();
        let max_key = new_targets
            .iter()
            .map(|idx| idx.index as u64)
            .max()
            .unwrap();
        let range = max_key.saturating_sub(min_key);

        let c = std::cmp::max(1, n / 4);
        let m = (c * 16 - 1) as u64;
        let multiplier = if range == 0 {
            0
        } else {
            (((m as u128) << 32) / (range as u128)) as u64
        };

        self.ws.prepare(c);

        self.ordered = Self::build_from_indices(new_targets, c, min_key, multiplier, &mut self.ws);
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

impl<'a, T> OrdexIter<'a, T> {
    #[inline(always)]
    pub fn next(&mut self) -> Option<&'a mut T> {
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
}

impl<T> OrdexArena<T> {
    pub(crate) const START_GENERATION: GenerationType = unsafe { GenerationType::new_unchecked(1) };

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
