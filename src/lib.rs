#![no_std]
#![cfg_attr(feature = "nightly", feature(allocator_api))]
// Lints: the root Cargo.toml's `[workspace.lints]`.

//! A generational handle table: [`Moorings`] owns its values, hands out one
//! [`Key`] for each, and never moves them.
//!
//! A mooring field has numbered buoys that boats tie up to and leave again;
//! your ticket says "buoy 12" - and if another boat is tied there by the time
//! you come back, your ticket is stale.
//!
//! # Core properties
//!
//! - **Keys are unique**: A [`Key`] is neither `Copy` nor `Clone`, and
//!   [`remove`](Moorings::remove) consumes it. Exactly one key exists per
//!   value, and none remains after removal.
//! - **Generational validity**: Each key carries its slot's generation, which
//!   increments on each reuse. A key to a removed value finds `None`, never a
//!   subsequent value in the same slot. Slots that exhaust their generations
//!   are retired.
//! - **Stable addresses**: The table grows by allocating chunk arrays (8, 16,
//!   32, ... slots), never moving existing entries. Values can be safely linked
//!   into intrusive data structures.
//! - **Fallible allocation**: Inserting into the table is strictly fallible
//!   ([`try_insert`](Moorings::try_insert)), returning [`InsertError`] on OOM
//!   rather than aborting.
//! - **Values that know their slot**:
//!   [`try_insert_with`](Moorings::try_insert_with) builds the value once its
//!   slot is known, so it can hold its own [`SlotId`] from the start.
//!
//! # Access modes
//!
//! - **Client access (via [`Key`])**: Handed out on insert; unique and
//!   invalidated on removal.
//! - **Owner access (via [`SlotId`])**: Opaque identifier for a slot, allowing
//!   the table's owner to reach or remove an entry without its key (e.g. an
//!   event queue processing an expired timer).
//!
//! # Features
#![doc = document_features::document_features!()]
#![doc(issue_tracker_base_url = "https://github.com/aqu-os/aquos/issues")]
// The README's code blocks run as doctests too (with alloc).
#![cfg_attr(all(doctest, feature = "alloc"), doc = include_str!("../README.md"))]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(test)]
extern crate std;

mod allocator;
mod generation;
mod key;
mod table;

#[cfg(feature = "alloc")]
pub use allocator::GlobalChunks;
#[cfg(feature = "nightly")]
pub use allocator::InAllocator;
pub use allocator::{ChunkAllocator, InsertError};
pub use key::{Key, SlotId};
pub use table::Moorings;
