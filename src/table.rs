//! The table: [`Moorings`].

use core::alloc::Layout;
use core::fmt;
use core::marker::PhantomData;
use core::ptr::NonNull;

#[cfg(feature = "alloc")]
use crate::GlobalChunks;
use crate::{ChunkAllocator, InsertError, Key, SlotId};

/// The first chunk's size, as a power of two: 8 slots. Each chunk after it is
/// twice the size of the one before.
const BASE_SHIFT: u32 = 3;
/// How many chunks a table can have - enough for every slot index there is.
const CHUNKS: usize = (usize::BITS - BASE_SHIFT) as usize;
/// The end of the free list.
const NO_SLOT: usize = usize::MAX;

/// One slot: a value, or room for one.
enum Slot<T> {
    /// Holds a value.
    Occupied {
        /// The generation the value was put in at - what its key carries.
        generation: u64,
        /// The value.
        value: T,
    },
    /// Empty.
    Vacant {
        /// The generation the next value put here gets.
        generation: u64,
        /// The next empty slot on the free list, or [`NO_SLOT`] (also for a
        /// retired slot, which isn't on the list at all).
        next_free:  usize,
    },
}

/// Which chunk slot `index` is in, and where in it.
///
/// Chunk `c` holds `8 << c` slots, starting at slot `8 * (2^c - 1)` - so
/// `index + 8`'s highest set bit names the chunk, and the bits below it the
/// offset.
const fn locate(index: usize) -> Option<(usize, usize)> {
    let Some(shifted) = index.checked_add(1 << BASE_SHIFT) else {
        return None;
    };
    let top_bit = usize::BITS - 1 - shifted.leading_zeros();
    Some(((top_bit - BASE_SHIFT) as usize, shifted - (1 << top_bit)))
}

/// How many slots chunk `chunk` holds.
const fn chunk_len(chunk: usize) -> usize {
    1 << (chunk as u32 + BASE_SHIFT)
}

/// A table that owns its values and hands out one [`Key`] for each - see the
/// crate docs.
///
/// Values never move: the table grows by adding chunks (8 slots, then 16,
/// 32, ...), so a value stays at the same address for as long as it's in the
/// table. Removed values' slots are reused, each reuse with a new generation,
/// so an old key never reaches the new value. A slot whose generation would
/// overflow is retired instead of reused.
pub struct Moorings<T, A: ChunkAllocator> {
    /// The chunks allocated so far, in order; chunk `c` holds `8 << c` slots.
    chunks: [Option<NonNull<Slot<T>>>; CHUNKS],
    /// How many slots have ever been written: `0..initialised`, all in
    /// allocated chunks.
    initialised: usize,
    /// The first empty slot to reuse, or [`NO_SLOT`].
    free: usize,
    /// How many values the table holds.
    len: usize,
    /// Where the chunks came from, to give them back to.
    alloc: A,
    /// Ownership of the `T`s.
    _owned: PhantomData<T>,
}

// SAFETY: owns its values (and drops them wherever it's dropped), like a
// `Vec`; the chunk pointers are only ever reached through it.
unsafe impl<T: Send, A: ChunkAllocator + Send> Send for Moorings<T, A> {}

// SAFETY: `&self` only reads (`get`, `contains`, `len`), like a `Vec`.
unsafe impl<T: Sync, A: ChunkAllocator + Sync> Sync for Moorings<T, A> {}

impl<T, A: ChunkAllocator> Moorings<T, A> {
    /// An empty table, allocating from `alloc` once something is inserted.
    pub const fn new_in(alloc: A) -> Self {
        Self {
            chunks: [None; CHUNKS],
            initialised: 0,
            free: NO_SLOT,
            len: 0,
            alloc,
            _owned: PhantomData,
        }
    }

    /// Puts `value` in the table and returns its key - or hands `value` back
    /// if there's no memory for it.
    pub fn try_insert(&mut self, value: T) -> Result<Key<T>, InsertError<T>> {
        let key = if self.free == NO_SLOT {
            self.insert_new(value)?
        } else {
            self.insert_reused(value)
        };
        self.len += 1;
        Ok(key)
    }

    /// The value `key` names, if it's still in the table.
    pub fn get(&self, key: &Key<T>) -> Option<&T> {
        // SAFETY: `slot` only hands out pointers to written slots, and
        // `&self` rules out a write while the reference lives.
        match unsafe { self.slot(key.index())?.as_ref() } {
            Slot::Occupied { generation, value }
                if *generation == key.generation() =>
            {
                Some(value)
            },
            _ => None,
        }
    }

    /// The value `key` names, mutably, if it's still in the table.
    pub fn get_mut(&mut self, key: &Key<T>) -> Option<&mut T> {
        // SAFETY: `slot` only hands out pointers to written slots, and
        // `&mut self` rules out any other access while the reference lives.
        match unsafe { self.slot(key.index())?.as_mut() } {
            Slot::Occupied { generation, value }
                if *generation == key.generation() =>
            {
                Some(value)
            },
            _ => None,
        }
    }

    /// Whether the value `key` names is still in the table.
    pub fn contains(&self, key: &Key<T>) -> bool {
        self.get(key).is_some()
    }

    /// Takes the value `key` names out of the table, if it's still there.
    /// The key is used up either way.
    pub fn remove(&mut self, key: Key<T>) -> Option<T> {
        self.take(key.index(), Some(key.generation()))
    }

    /// Whatever value is in `slot` now - see [`SlotId`](crate::SlotId).
    pub fn get_slot(&self, slot: SlotId<T>) -> Option<&T> {
        // SAFETY: `slot` only hands out pointers to written slots, and
        // `&self` rules out a write while the reference lives.
        match unsafe { self.slot(slot.index())?.as_ref() } {
            Slot::Occupied { value, .. } => Some(value),
            Slot::Vacant { .. } => None,
        }
    }

    /// Whatever value is in `slot` now, mutably - see
    /// [`SlotId`](crate::SlotId).
    pub fn get_slot_mut(&mut self, slot: SlotId<T>) -> Option<&mut T> {
        // SAFETY: `slot` only hands out pointers to written slots, and
        // `&mut self` rules out any other access while the reference lives.
        match unsafe { self.slot(slot.index())?.as_mut() } {
            Slot::Occupied { value, .. } => Some(value),
            Slot::Vacant { .. } => None,
        }
    }

    /// Takes whatever value is in `slot` now out of the table, without its
    /// key - see [`SlotId`](crate::SlotId). Its key, wherever it is, finds
    /// nothing from then on.
    pub fn remove_slot(&mut self, slot: SlotId<T>) -> Option<T> {
        self.take(slot.index(), None)
    }

    /// Takes the value out of slot `index` - if it holds one, and (if
    /// `generation` is given) one put there at that generation.
    fn take(&mut self, index: usize, generation: Option<u64>) -> Option<T> {
        let mut slot = self.slot(index)?;
        // SAFETY: a written slot (see `slot`), and `&mut self` rules out
        // any other access.
        let slot = unsafe { slot.as_mut() };
        let current = match slot {
            Slot::Occupied {
                generation: current,
                ..
            } if generation.is_none_or(|wanted| wanted == *current) => *current,
            _ => return None,
        };

        // The next value here gets a new generation - or, if there's none
        // left, the slot is retired: never reused, so no old key can match.
        let next = current.checked_add(1);
        let vacant = Slot::Vacant {
            generation: next.unwrap_or(current),
            next_free:  if next.is_some() { self.free } else { NO_SLOT },
        };
        let Slot::Occupied { value, .. } = core::mem::replace(slot, vacant)
        else {
            unreachable!("checked to be occupied just above");
        };
        if next.is_some() {
            self.free = index;
        }
        self.len -= 1;
        Some(value)
    }

    /// How many values the table holds.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether the table holds no values.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Slot `index`, if it has ever been written.
    fn slot(&self, index: usize) -> Option<NonNull<Slot<T>>> {
        if index >= self.initialised {
            return None;
        }
        let (chunk, offset) = locate(index)?;
        let first = self.chunks[chunk]?;
        // SAFETY: slot `index` was written, so its chunk holds at least
        // `offset + 1` slots.
        Some(unsafe { first.add(offset) })
    }

    /// Puts `value` in the free list's first slot.
    fn insert_reused(&mut self, value: T) -> Key<T> {
        let index = self.free;
        let mut slot =
            self.slot(index).expect("the free list holds written slots");
        // SAFETY: a written slot (see `slot`), and `&mut self` rules out
        // any other access.
        let slot = unsafe { slot.as_mut() };
        let Slot::Vacant {
            generation,
            next_free,
        } = *slot
        else {
            unreachable!("the free list holds only empty slots");
        };
        *slot = Slot::Occupied { generation, value };
        self.free = next_free;
        Key::new(index, generation)
    }

    /// Puts `value` in the first never-used slot, allocating its chunk if
    /// it's the first slot of one.
    fn insert_new(&mut self, value: T) -> Result<Key<T>, InsertError<T>> {
        let index = self.initialised;
        // Every index's chunk is below `CHUNKS` (see the tests), so `chunk`
        // always indexes `chunks`.
        let Some((chunk, offset)) = locate(index) else {
            return Err(InsertError::new(value));
        };
        let first = match self.chunks[chunk] {
            Some(first) => first,
            None => {
                let Ok(layout) = Layout::array::<Slot<T>>(chunk_len(chunk))
                else {
                    return Err(InsertError::new(value));
                };
                // `Slot` holds a `u64`, so the layout is never zero-sized.
                let Some(first) = self.alloc.allocate(layout) else {
                    return Err(InsertError::new(value));
                };
                let first = first.cast();
                self.chunks[chunk] = Some(first);
                first
            },
        };
        // SAFETY: `offset` is within chunk `chunk` (see `locate`), which is
        // allocated, and slot `index` has never been written.
        unsafe {
            first.add(offset).write(Slot::Occupied {
                generation: 0,
                value,
            });
        }
        self.initialised += 1;
        Ok(Key::new(index, 0))
    }
}

#[cfg(feature = "alloc")]
impl<T> Moorings<T, GlobalChunks> {
    /// An empty table on the global allocator.
    pub const fn new() -> Self {
        Self::new_in(GlobalChunks)
    }
}

#[cfg(feature = "alloc")]
impl<T> Default for Moorings<T, GlobalChunks> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, A: ChunkAllocator> Drop for Moorings<T, A> {
    /// Drops every value still in the table, then gives the chunks back.
    fn drop(&mut self) {
        for index in 0..self.initialised {
            if let Some(slot) = self.slot(index) {
                // SAFETY: a written slot, dropped exactly once, here.
                unsafe { slot.drop_in_place() };
            }
        }
        for (chunk, first) in self.chunks.iter().enumerate() {
            // The layout was valid when the chunk was allocated, so it is now.
            let (Some(first), Ok(layout)) =
                (*first, Layout::array::<Slot<T>>(chunk_len(chunk)))
            else {
                continue;
            };
            // SAFETY: allocated from `alloc` with this layout in
            // `insert_new`, and given back only here.
            unsafe { self.alloc.deallocate(first.cast(), layout) };
        }
    }
}

// How full it is; the values needn't be `Debug`.
impl<T, A: ChunkAllocator> fmt::Debug for Moorings<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Moorings")
            .field("len", &self.len)
            .field("slots", &self.initialised)
            .finish_non_exhaustive()
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    #[test]
    fn slots_map_onto_doubling_chunks() {
        assert_eq!(locate(0), Some((0, 0)));
        assert_eq!(locate(7), Some((0, 7)));
        assert_eq!(locate(8), Some((1, 0)));
        assert_eq!(locate(23), Some((1, 15)));
        assert_eq!(locate(24), Some((2, 0)));
        assert_eq!(locate(usize::MAX), None);
        // The last index `locate` takes lands in the last chunk there is room
        // for: `CHUNKS` is exactly enough.
        let last = usize::MAX - (1 << BASE_SHIFT);
        assert_eq!(locate(last).map(|(chunk, _)| chunk), Some(CHUNKS - 1));
        assert_eq!(chunk_len(0), 8);
        assert_eq!(chunk_len(2), 32);
    }

    #[test]
    fn a_slot_out_of_generations_is_retired() {
        let mut table = Moorings::<u32, _>::new();
        let key = table.try_insert(1).expect("memory");
        // Fast-forward slot 0 to its last generation.
        let mut slot = table.slot(key.index()).expect("written");
        // SAFETY: a written slot, and nothing else uses the table meanwhile.
        if let Slot::Occupied { generation, .. } = unsafe { slot.as_mut() } {
            *generation = u64::MAX;
        }
        let last = Key::new(key.index(), u64::MAX);

        assert_eq!(table.remove(last), Some(1));
        let next = table.try_insert(2).expect("memory");
        assert_eq!(next.index(), 1, "slot 0 is retired, not reused");
        assert_eq!(table.get(&Key::new(0, u64::MAX)), None);
        assert_eq!(table.len(), 1);
    }
}
