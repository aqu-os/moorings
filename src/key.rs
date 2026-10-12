//! The handle a table hands out for each value: [`Key`].

use core::fmt;
use core::marker::PhantomData;

use crate::generation::Generation;
use crate::table_id::TableId;

/// The one handle to a value in a [`Moorings<T, _>`](crate::Moorings): which
/// slot it's in, and that slot's generation when it was put there.
///
/// Neither `Copy` nor `Clone`, and [`remove`](crate::Moorings::remove)
/// consumes it - so each value has exactly one key, and none is left once the
/// value is gone. It's typed by what the table holds, so it can't name a
/// value in a table of something else. (Two tables of the same `T` share a
/// key type: a key used on the wrong one finds nothing, or another value -
/// never undefined behaviour.)
///
/// A key outlives its value only when the table's owner removes the value
/// without it ([`Moorings::remove_slot`](crate::Moorings::remove_slot)): the
/// key then finds nothing, never a value put in the same slot later.
// The examples use the global allocator, so they're only there (and only
// tested) with the `alloc` feature.
#[cfg_attr(feature = "alloc", doc = "")]
#[cfg_attr(feature = "alloc", doc = include_str!("../docs/key.md"))]
pub struct Key<T> {
    /// The slot the value is in.
    index: usize,
    /// The slot's generation when the value was put there.
    generation: Generation,
    /// Which table this key belongs to - checked on every access in debug
    /// builds to catch cross-table misuse early.
    #[cfg(debug_assertions)]
    table_id: TableId,
    /// Ties the key to `T` without implying ownership, and opts out of the
    /// automatic `Send`/`Sync` derivation so the explicit impls below control
    /// it instead.
    _value: PhantomData<*const T>,
}

// SAFETY: `Key<T>` holds no `T` - just a slot index and a generation counter,
// both plain integers. There is nothing to race on or move unsafely, regardless
// of what `T` itself is. `Moorings<T, _>` still gates on `T: Send`/`T: Sync`
// because it *owns* the values; the key is merely a lookup token.
unsafe impl<T> Send for Key<T> {}
unsafe impl<T> Sync for Key<T> {}

impl<T> Key<T> {
    /// The slot this key's value is in - for the table's owner to reach the
    /// value without the key (see [`SlotId`]).
    pub const fn slot(&self) -> SlotId<T> {
        SlotId::new(
            self.index,
            #[cfg(debug_assertions)]
            self.table_id,
        )
    }

    /// The key for slot `index` at `generation` in the table identified by
    /// `table_id`.
    pub(crate) const fn new(
        index: usize,
        generation: Generation,
        #[cfg(debug_assertions)] table_id: TableId,
    ) -> Self {
        Self {
            index,
            generation,
            #[cfg(debug_assertions)]
            table_id,
            _value: PhantomData,
        }
    }

    /// The slot the value is in.
    pub(crate) const fn index(&self) -> usize {
        self.index
    }

    /// The slot's generation when the value was put there.
    pub(crate) const fn generation(&self) -> Generation {
        self.generation
    }

    /// Panics in debug builds if `self` does not belong to the table
    /// identified by `id`. No-op in release builds.
    #[inline]
    pub(crate) fn assert_table(&self, #[cfg(debug_assertions)] id: TableId) {
        #[cfg(debug_assertions)]
        assert!(self.table_id == id, "key used on the wrong Moorings table");
    }
}

/// A slot in a [`Moorings<T, _>`](crate::Moorings), without a generation:
/// whatever value is in it *now*.
///
/// For the table's owner, not for whoever holds a value's [`Key`]: the owner
/// may need to reach or remove a value without its key - a timer queue
/// taking a timer that just fired off the queue, while the timer's key is
/// still out there with whoever started it. That key then simply finds
/// nothing. `Copy`, so the owner can keep it next to the value (in an
/// intrusive list node, say).
pub struct SlotId<T> {
    /// The slot.
    index:    usize,
    /// Which table this slot id belongs to - checked on every access in debug
    /// builds to catch cross-table misuse early.
    #[cfg(debug_assertions)]
    table_id: TableId,
    /// Ties the slot id to `T` without implying ownership, and opts out of
    /// the automatic `Send`/`Sync` derivation so the explicit impls below
    /// control it instead.
    _value:   PhantomData<*const T>,
}

// SAFETY: same reasoning as for `Key<T>`: a `SlotId<T>` is just an integer
// index with no `T` data inside.
unsafe impl<T> Send for SlotId<T> {}
unsafe impl<T> Sync for SlotId<T> {}

impl<T> SlotId<T> {
    /// Slot `index` in the table identified by `table_id`.
    pub(crate) const fn new(
        index: usize,
        #[cfg(debug_assertions)] table_id: TableId,
    ) -> Self {
        Self {
            index,
            #[cfg(debug_assertions)]
            table_id,
            _value: PhantomData,
        }
    }

    /// The slot.
    pub(crate) const fn index(self) -> usize {
        self.index
    }

    /// Panics in debug builds if `self` does not belong to the table
    /// identified by `id`. No-op in release builds.
    #[inline]
    pub(crate) fn assert_table(&self, #[cfg(debug_assertions)] id: TableId) {
        #[cfg(debug_assertions)]
        assert!(
            self.table_id == id,
            "slot id used on the wrong Moorings table"
        );
    }
}

// By hand: derived ones would need `T: Clone`/`T: Copy`.
impl<T> Clone for SlotId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for SlotId<T> {}

impl<T> PartialEq for SlotId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for SlotId<T> {}

impl<T> core::hash::Hash for SlotId<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

// Meant to be read: "slot 3".
impl<T> fmt::Display for SlotId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "slot {}", self.index)
    }
}

impl<T> fmt::Debug for SlotId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl<T> PartialEq for Key<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}

impl<T> Eq for Key<T> {}

impl<T> core::hash::Hash for Key<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}

// Meant to be read: "slot 3, generation 2".
impl<T> fmt::Display for Key<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "slot {}, generation {}", self.index, self.generation)
    }
}

impl<T> fmt::Debug for Key<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
