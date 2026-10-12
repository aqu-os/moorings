//! The handle a table hands out for each value: [`Key`].

use core::fmt;
use core::marker::PhantomData;

use crate::generation::Generation;

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
    /// What the table holds, without affecting `Send`/`Sync`.
    ///
    /// `PhantomData<fn() -> T>` (not `PhantomData<T>`) keeps `Key<T>:
    /// Send + Sync` for all `T`, even non-`Send`/non-`Sync` ones like
    /// `Cell<u32>`. That's correct: a key holds no `T` data - just two
    /// integers - so it's safe to move or share across threads by itself.
    /// `Moorings<T, _>` still requires `T: Send`/`T: Sync` because it
    /// actually owns the values.
    _value: PhantomData<fn() -> T>,
}

impl<T> Key<T> {
    /// The slot this key's value is in - for the table's owner to reach the
    /// value without the key (see [`SlotId`]).
    pub const fn slot(&self) -> SlotId<T> {
        SlotId::new(self.index)
    }

    /// The key for slot `index` at `generation`.
    pub(crate) const fn new(index: usize, generation: Generation) -> Self {
        Self {
            index,
            generation,
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
    index:  usize,
    /// What the table holds, without affecting `Send`/`Sync`. Same reasoning
    /// as [`Key`]: a slot id holds no `T` data, so it's unconditionally
    /// `Send + Sync`.
    _value: PhantomData<fn() -> T>,
}

impl<T> SlotId<T> {
    /// Slot `index`.
    pub(crate) const fn new(index: usize) -> Self {
        Self {
            index,
            _value: PhantomData,
        }
    }

    /// The slot.
    pub(crate) const fn index(self) -> usize {
        self.index
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
