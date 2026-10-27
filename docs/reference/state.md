# `src/state.rs`

[← docs index](../README.md)

Plain world-state data types. No validation and no JSON. Rules live in `engine.rs`.

## Type aliases

| Alias | Type | Meaning |
|---|---|---|
| `VesselId` | `String` | vessel id from `init` |
| `Location` | `usize` | index into `travel_times` |
| `Tick` | `u64` | simulation time |
| `Inventory` | `IndexMap<Location, u32>` | cargo quantity by destination, in insertion order |

## Types

| Type | Fields | Notes |
|---|---|---|
| `VesselStatus` | `Idle { location }`, `Docked { location }`, `Transit { from, to, arrives_at_tick }` | |
| `Vessel` | `status`, `cargo: Inventory` | |
| `World` | `tick`, `vessels: IndexMap<VesselId, Vessel>`, `docks: Vec<Inventory>` | one per committed/planned/history entry; `Clone` |
| `Config` | `travel_times: Vec<Vec<u64>>`, `dock_throughput: Option<Vec<u32>>` | fixed at init |

`Config` methods:

- `num_locations()` returns `travel_times.len()`
- `cargo_enabled()` returns `dock_throughput.is_some()`

## Cargo helpers

Both are `#[must_use]` and leave the inventory untouched on failure.

| Function | Returns `false` when |
|---|---|
| `add_cargo(inv, dest, qty)` | the new total would overflow `u32` |
| `remove_cargo(inv, dest, qty)` | there's less than `qty` for `dest` (removes the entry when it hits 0) |
