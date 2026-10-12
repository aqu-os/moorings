moorings
========

[![docs.rs](https://img.shields.io/docsrs/moorings)](https://docs.rs/moorings)
[![crates.io](https://img.shields.io/crates/v/moorings)](https://crates.io/crates/moorings)
[![Tests](https://github.com/aqu-os/aquos/actions/workflows/tests.yml/badge.svg)](https://github.com/aqu-os/aquos/actions/workflows/tests.yml)
[![Clippy](https://github.com/aqu-os/aquos/actions/workflows/clippy.yml/badge.svg)](https://github.com/aqu-os/aquos/actions/workflows/clippy.yml)
[![Rustfmt](https://github.com/aqu-os/aquos/actions/workflows/rustfmt.yml/badge.svg)](https://github.com/aqu-os/aquos/actions/workflows/rustfmt.yml)
[![Coverage](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Faqu-os.github.io%2Faquos%2Fcoverage%2Fsummary.json&query=%24.suites%5B%27moorings%27%5D.lines&suffix=%25&label=coverage)](https://aqu-os.github.io/aquos/coverage/moorings/html/index.html)

## Usage

Add moorings to your project e.g. via ```cargo add moorings```.

```rust
use moorings::Moorings;

let mut boats = Moorings::new();
let ticket = boats.try_insert("Seaweed").expect("memory for a mooring");
assert_eq!(boats.get(&ticket), Some(&"Seaweed"));

assert_eq!(boats.remove(ticket), Some("Seaweed"));
assert!(boats.is_empty());
```

## About

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
  any `core::alloc::Allocator` (`InAllocator`, with the `allocator-api` feature).

Or specify an allocator explicitly:

```rust
use moorings::{GlobalChunks, Moorings};

let mut boats = Moorings::new_in(GlobalChunks);
let _ticket = boats.try_insert("Seaweed");
```
