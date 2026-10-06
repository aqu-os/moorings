//! Where a table's chunks come from: [`ChunkAllocator`], and its ready-made
//! implementations.

use core::alloc::Layout;
use core::fmt;
use core::ptr::NonNull;

/// Where a [`Moorings`](crate::Moorings) table's chunks come from - a heap,
/// an arena, a region set aside at boot.
///
/// Allocation is always fallible: a failed allocation hands the value being
/// inserted back ([`InsertError`]) rather than aborting.
///
/// Ready-made: `GlobalChunks` (the global allocator; `alloc` feature) and
/// `InAllocator` (any `core::alloc::Allocator`; `nightly` feature).
///
/// # Safety
///
/// Memory [`allocate`](Self::allocate) returns must be valid for `layout`,
/// and used by nothing else until it's passed to
/// [`deallocate`](Self::deallocate) with the same layout.
pub unsafe trait ChunkAllocator {
    /// Memory for `layout` (whose size isn't zero), or `None` if there is
    /// none.
    fn allocate(&self, layout: Layout) -> Option<NonNull<u8>>;

    /// Gives back `ptr`.
    ///
    /// # Safety
    ///
    /// `ptr` must come from [`allocate`](Self::allocate) on this allocator,
    /// with this `layout`, and not have been given back already.
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout);
}

/// The global allocator (`alloc::alloc::alloc`), through its fallible raw
/// interface - which works on stable Rust, and doesn't need a global
/// out-of-memory handler.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, Copy, Default)]
pub struct GlobalChunks;

// SAFETY: forwards to the global allocator, whose contract is this one.
#[cfg(feature = "alloc")]
unsafe impl ChunkAllocator for GlobalChunks {
    fn allocate(&self, layout: Layout) -> Option<NonNull<u8>> {
        // SAFETY: `layout`'s size isn't zero (the trait's contract).
        NonNull::new(unsafe { alloc::alloc::alloc(layout) })
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: forwarded from this function's contract.
        unsafe { alloc::alloc::dealloc(ptr.as_ptr(), layout) }
    }
}

/// Any [`core::alloc::Allocator`] as a [`ChunkAllocator`].
#[cfg(feature = "nightly")]
#[derive(Debug, Clone, Copy, Default)]
pub struct InAllocator<A>(pub A);

// SAFETY: forwards to `A`, whose contract (`Allocator`'s) is this one.
#[cfg(feature = "nightly")]
unsafe impl<A: core::alloc::Allocator> ChunkAllocator for InAllocator<A> {
    fn allocate(&self, layout: Layout) -> Option<NonNull<u8>> {
        self.0.allocate(layout).ok().map(NonNull::cast)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: forwarded from this function's contract.
        unsafe { self.0.deallocate(ptr, layout) }
    }
}

/// [`Moorings::try_insert`](crate::Moorings::try_insert) found no room: here's
/// the value back.
pub struct InsertError<T> {
    /// The value that wasn't inserted.
    value: T,
}

impl<T> InsertError<T> {
    /// Wraps the value that wasn't inserted.
    pub(crate) const fn new(value: T) -> Self {
        Self { value }
    }

    /// The value that wasn't inserted.
    pub fn into_inner(self) -> T {
        self.value
    }
}

// Not the value: it needn't be `Debug`, and the error is about the table.
impl<T> fmt::Debug for InsertError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InsertError").finish_non_exhaustive()
    }
}

impl<T> fmt::Display for InsertError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("out of memory for another mooring")
    }
}

impl<T> core::error::Error for InsertError<T> {}
