#![no_std]
#![cfg_attr(feature = "nightly", feature(allocator_api))]
// Lints: the root Cargo.toml's `[workspace.lints]`.

//! A generational handle table: [`Moorings`] owns its values, hands out one
//! [`Key`] for each, and never moves them.
// The README is the crate's real introduction; its example uses the global
// allocator, so it's only there (and only tested) with the `alloc` feature.
#![cfg_attr(feature = "alloc", doc = "")]
#![cfg_attr(feature = "alloc", doc = include_str!("../README.md"))]
#![doc(issue_tracker_base_url = "https://github.com/aqu-os/aquos/issues")]

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
