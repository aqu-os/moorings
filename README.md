# moorings

A `no_std` generational handle table. A mooring field has numbered buoys
that boats tie up to and leave again; your ticket says "buoy 12" - and if
another boat is tied there by the time you come back, your ticket is stale.

[`Moorings`] owns its values and hands out a [`Key`] for each:

- **Keys are unique.** A key is neither `Copy` nor `Clone`, and removing a
  value consumes its key - so there's exactly one key per value, and none
  left over once it's gone.
- **Keys can't outlive what they name.** Each key carries its slot's
  *generation*, which goes up every time the slot is reused: a key to a
  removed value finds nothing (`None`), never the value that took its place.
- **Values never move.** The table grows by adding chunks, never by moving
  what's there, so a value's address stays the same for as long as it's in
  the table - it can be linked into an intrusive list or tree.
- **Allocation is always fallible.** Running out of memory hands the value
  back, instead of aborting. The memory comes from a [`ChunkAllocator`]: the
  global allocator ([`GlobalChunks`], with the default `alloc` feature) or
  any `Allocator` (`InAllocator`, with the `nightly` feature).

```rust
use moorings::{GlobalChunks, Moorings};

let mut boats = Moorings::new_in(GlobalChunks);
let ticket = boats.try_insert("Seaweed").expect("memory for a mooring");
assert_eq!(boats.get(&ticket), Some(&"Seaweed"));

assert_eq!(boats.remove(ticket), Some("Seaweed"));
assert!(boats.is_empty());
```
