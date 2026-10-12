//! A per-table identity token for catching cross-table key misuse in debug
//! builds: [`TableId`].

/// A unique identity for one [`Moorings`](crate::Moorings) table.
///
/// In debug builds this is a `u32` drawn from a global counter, stored in
/// every [`Key`](crate::Key) and [`SlotId`](crate::SlotId) handed out by that
/// table. Access methods panic if a key or slot id from a different table is
/// used. In release builds the type is a zero-sized no-op.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct TableId(Inner);

impl TableId {
    /// Allocates a fresh id - one per new table.
    pub(crate) fn new() -> Self {
        Self(Inner::new())
    }
}

// --- debug build -----------------------------------------------------------

#[cfg(debug_assertions)]
/// Debug-build inner: a `u32` counter.
mod inner {
    use core::sync::atomic::{AtomicU32, Ordering};

    /// The next id to hand out.
    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// The real id: a `u32` counter value.
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) struct Inner(u32);

    impl Inner {
        /// Allocates the next id.
        pub(super) fn new() -> Self {
            // Wrapping add: after 2^32 tables the ids repeat, which is
            // astronomically unlikely in practice, and the worst outcome
            // (a missed cross-table panic) is the same as a release build.
            Self(NEXT.fetch_add(1, Ordering::Relaxed))
        }
    }
}

// --- release build (zero-cost) ---------------------------------------------

#[cfg(not(debug_assertions))]
/// Release-build inner: a ZST, zero bytes.
mod inner {
    /// The zero-cost stand-in: a ZST.
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub(super) struct Inner;

    impl Inner {
        /// No-op constructor.
        pub(super) fn new() -> Self {
            Self
        }
    }
}

use inner::Inner;
