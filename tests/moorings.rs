//! A table's behaviour from the outside: keys, generations, values that never
//! move, values dropped exactly once, and running out of memory.
#![cfg_attr(feature = "nightly", feature(allocator_api))]

use std::alloc::Layout;
use std::cell::Cell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::vec::Vec;

use moorings::{ChunkAllocator, GlobalChunks, Moorings};

#[test]
fn a_value_is_found_by_its_key_until_removed() {
    let mut table = Moorings::new();
    let key = table.try_insert("buoy").expect("memory");
    assert_eq!(table.len(), 1);
    assert!(!table.is_empty());
    assert!(table.contains(&key));
    assert_eq!(table.get(&key), Some(&"buoy"));

    *table.get_mut(&key).expect("still there") = "boat";
    assert_eq!(table.remove(key), Some("boat"));
    assert!(table.is_empty());
}

#[test]
fn a_reused_slot_gets_a_new_generation() {
    let mut table = Moorings::new();
    let first = table.try_insert(1).expect("memory");
    assert_eq!(first.to_string(), "slot 0, generation 1");
    table.remove(first);

    let second = table.try_insert(2).expect("memory");
    assert_eq!(format!("{second:?}"), "slot 0, generation 2");
    assert_eq!(table.get(&second), Some(&2));
}

#[test]
fn the_owner_can_reach_and_remove_a_value_without_its_key() {
    let mut table = Moorings::new();
    let key = table.try_insert(1).expect("memory");
    let slot = key.slot();
    assert_eq!(slot.to_string(), "slot 0");
    assert_eq!(format!("{slot:?}"), "slot 0");

    assert_eq!(table.get_slot(slot), Some(&1));
    *table.get_slot_mut(slot).expect("there") += 1;
    assert_eq!(table.get(&key), Some(&2), "the same value");

    assert_eq!(table.remove_slot(slot), Some(2));
    assert_eq!(table.get_slot(slot), None);
    assert_eq!(table.get_slot_mut(slot), None);
    assert_eq!(table.remove_slot(slot), None);
    assert!(table.is_empty());
    assert_eq!(table.get(&key), None, "and its key finds nothing either");
}

#[test]
fn each_slot_reaches_its_own_value() {
    let mut table = Moorings::new();
    let keys: Vec<_> = (0..3)
        .map(|i| table.try_insert(i * 10).expect("memory"))
        .collect();
    let slots: Vec<_> = keys.iter().map(|key| key.slot()).collect();
    assert!(slots[1] != slots[2]);
    assert_eq!(slots[2].to_string(), "slot 2");

    assert_eq!(table.get_slot(slots[2]), Some(&20));
    assert_eq!(table.remove_slot(slots[1]), Some(10));
    assert_eq!(table.get_slot(slots[0]), Some(&0), "untouched");
    assert_eq!(table.get_slot(slots[2]), Some(&20), "untouched");
}

#[test]
fn a_key_outliving_its_value_never_finds_the_next_one() {
    let mut table = Moorings::new();
    let stale = table.try_insert("fired").expect("memory");
    // The owner takes it out without the key (a timer that fired, say)...
    assert_eq!(table.remove_slot(stale.slot()), Some("fired"));
    // ...and the slot is reused for something else.
    let next = table.try_insert("started").expect("memory");
    assert!(next.slot() == stale.slot(), "the same slot, reused");

    assert_eq!(table.get(&stale), None);
    assert!(!table.contains(&stale));
    assert_eq!(table.get_mut(&stale), None);
    assert_eq!(table.remove(stale), None);
    assert_eq!(table.get(&next), Some(&"started"), "untouched");
    assert_eq!(table.len(), 1);
}

#[test]
fn freed_slots_are_reused_last_freed_first() {
    let mut table = Moorings::new();
    let a = table.try_insert('a').expect("memory");
    let b = table.try_insert('b').expect("memory");
    let _c = table.try_insert('c').expect("memory");
    table.remove(a);
    table.remove(b);

    assert_eq!(
        table.try_insert('d').expect("memory").to_string(),
        "slot 1, generation 2"
    );
    assert_eq!(
        table.try_insert('e').expect("memory").to_string(),
        "slot 0, generation 2"
    );
    assert_eq!(
        table.try_insert('f').expect("memory").to_string(),
        "slot 3, generation 1"
    );
}

#[test]
fn values_never_move_while_the_table_grows() {
    let mut table = Moorings::new();
    let first = table.try_insert(0usize).expect("memory");
    let address = table.get(&first).expect("there") as *const usize;

    let keys: Vec<_> = (1..1000)
        .map(|i| table.try_insert(i).expect("memory"))
        .collect();

    assert_eq!(table.get(&first).expect("there") as *const usize, address);
    for (i, key) in keys.iter().enumerate() {
        assert_eq!(table.get(key), Some(&(i + 1)));
    }
    assert_eq!(table.len(), 1000);
}

/// A value that counts being dropped, on a counter of its test's own.
struct Counted(&'static AtomicUsize);

impl Drop for Counted {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

/// The global allocator, counting what's allocated and given back, refusing
/// once `limit` allocations have been made.
struct CountingChunks {
    /// Allocations made.
    allocated: Cell<usize>,
    /// Allocations given back.
    freed: Cell<usize>,
    /// How many allocations to allow.
    limit: Cell<usize>,
}

impl CountingChunks {
    /// Allows `limit` allocations.
    fn new(limit: usize) -> Self {
        Self {
            allocated: Cell::new(0),
            freed: Cell::new(0),
            limit: Cell::new(limit),
        }
    }
}

// SAFETY: forwards to `GlobalChunks`, or hands out nothing.
unsafe impl ChunkAllocator for &CountingChunks {
    fn allocate(&self, layout: Layout) -> Option<NonNull<u8>> {
        if self.allocated.get() == self.limit.get() {
            return None;
        }
        self.allocated.set(self.allocated.get() + 1);
        GlobalChunks.allocate(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.freed.set(self.freed.get() + 1);
        // SAFETY: forwarded from this function's contract.
        unsafe { GlobalChunks.deallocate(ptr, layout) }
    }
}

#[test]
fn dropping_the_table_drops_what_it_holds_and_frees_every_chunk() {
    static DROPPED: AtomicUsize = AtomicUsize::new(0);
    let chunks = CountingChunks::new(usize::MAX);
    let mut table = Moorings::new_in(&chunks);

    // 8 + 16 slots: two chunks.
    let mut keys: Vec<_> = (0..20)
        .map(|_| table.try_insert(Counted(&DROPPED)).expect("memory"))
        .collect();
    // Removed ones are dropped on removal, not again with the table.
    for key in keys.drain(..5) {
        drop(table.remove(key));
    }
    assert_eq!(DROPPED.load(Ordering::Relaxed), 5);

    drop(table);
    assert_eq!(DROPPED.load(Ordering::Relaxed), 20);
    assert_eq!((chunks.allocated.get(), chunks.freed.get()), (2, 2));
}

#[test]
fn out_of_memory_hands_the_value_back() {
    let chunks = CountingChunks::new(0);
    let mut table = Moorings::new_in(&chunks);

    let error = table.try_insert(42).expect_err("no memory");
    assert_eq!(error.to_string(), "out of memory for another mooring");
    assert_eq!(format!("{error:?}"), "InsertError { .. }");
    assert_eq!(error.into_inner(), 42);
    assert!(table.is_empty());
}

#[test]
fn running_out_of_memory_later_leaves_what_is_there_intact() {
    let chunks = CountingChunks::new(1);
    let mut table = Moorings::new_in(&chunks);
    let keys: Vec<_> = (0..8)
        .map(|i| table.try_insert(i).expect("the first chunk"))
        .collect();

    // The ninth needs a second chunk.
    assert_eq!(
        table.try_insert(8).map_err(|e| e.into_inner()).err(),
        Some(8)
    );
    assert_eq!(table.len(), 8);
    for (i, key) in keys.iter().enumerate() {
        assert_eq!(table.get(key), Some(&i));
    }

    // Once there's memory again, the table grows as if nothing happened.
    chunks.limit.set(2);
    let ninth = table.try_insert(8).expect("memory again");
    assert_eq!(ninth.to_string(), "slot 8, generation 1");
}

/// A value that knows its own slot.
struct Tied(&'static str, moorings::SlotId<Tied>);

#[test]
fn a_value_can_be_built_knowing_its_slot() {
    let mut table = Moorings::new();
    let first = table.try_insert_with("first", Tied).expect("memory");
    let second = table.try_insert_with("second", Tied).expect("memory");
    let tied = table.get(&second).expect("there");
    assert_eq!((tied.0, tied.1), ("second", second.slot()));

    // A reused slot is told too.
    let reused = first.slot();
    table.remove(first);
    let third = table.try_insert_with("third", Tied).expect("memory");
    assert_eq!(third.slot(), reused);
    assert_eq!(table.get(&third).expect("there").1, reused);
}

#[test]
fn out_of_memory_hands_the_parts_back_unbuilt() {
    let chunks = CountingChunks::new(0);
    let mut table = Moorings::<u32, _>::new_in(&chunks);

    let error = table
        .try_insert_with(42, |_, _| unreachable!("no slot to build for"))
        .expect_err("no memory");
    assert_eq!(error.into_inner(), 42);
    assert!(table.is_empty());
}

#[test]
fn debug_shows_how_full_it_is() {
    let mut table = Moorings::new();
    let key = table.try_insert(1).expect("memory");
    table.try_insert(2).expect("memory");
    table.remove(key);
    assert_eq!(format!("{table:?}"), "Moorings { len: 1, slots: 2, .. }");
    assert_eq!(
        format!("{:?}", Moorings::<u8, _>::default()),
        "Moorings { len: 0, slots: 0, .. }"
    );
}

#[cfg(feature = "nightly")]
#[test]
fn any_allocator_can_back_the_table() {
    use std::alloc::Global;

    use moorings::InAllocator;

    let mut table = Moorings::new_in(InAllocator(Global));
    let key = table.try_insert("anchored").expect("memory");
    assert_eq!(table.remove(key), Some("anchored"));
}

#[test]
#[expect(
    clippy::disallowed_types,
    reason = "a host test of `Hash`: a hashing set is what it needs"
)]
fn key_and_slot_support_equality_and_hashing() {
    use std::collections::HashSet;

    let mut table = Moorings::new();
    let k1 = table.try_insert(10).expect("memory");
    let k2 = table.try_insert(20).expect("memory");

    assert_eq!(k1, k1);
    assert_ne!(k1, k2);

    let s1 = k1.slot();
    let s2 = k2.slot();
    assert_eq!(s1, s1);
    assert_ne!(s1, s2);

    let mut slot_set = HashSet::new();
    slot_set.insert(s1);
    slot_set.insert(s2);
    assert_eq!(slot_set.len(), 2);
    assert!(slot_set.contains(&s1));

    let mut key_set = HashSet::new();
    key_set.insert(&k1);
    key_set.insert(&k2);
    assert_eq!(key_set.len(), 2);
    assert!(key_set.contains(&k1));
}
