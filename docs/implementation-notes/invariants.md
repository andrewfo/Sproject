# Invariants

[← docs index](../README.md)

These hold after every command. The first three are also listed at the top of `src/engine.rs`.

| # | Invariant | Enforced by |
|---|---|---|
| 1 | Right after a tick, `planned == committed`. | `Sim::tick` assigns both from the same `next` |
| 2 | Idle/docked checks read **committed**; cargo checks read **planned**. | `committed_vessel` helper; `transfer` works on a scratch copy of planned |
| 3 | A failed command leaves state exactly as it was. | Handlers validate fully before mutating, or work on a scratch copy |
| 4 | `history[t]` is the committed world at tick `t`, and `history.len() == tick + 1`. | `init` seeds it, `tick` pushes |
| 5 | Every vessel location, transit endpoint, and manifest location is in `0..N`. | `validate_*`, `check_location` |
| 6 | Vessel and cargo order in output is insertion order. | `IndexMap` + `serde_json/preserve_order` |
| 7 | In base mode no `cargo` keys appear in output. | `render_world(.., cargo_enabled)` |
| 8 | One non-blank input line → exactly one output line. | `main.rs` loop, `render_response` |
| 9 | Tick arithmetic never overflows silently. | `checked_add` in `set_destination`, `ArrivalOverflow` error |
| 10 | Cargo quantities never overflow or go negative. | `add_cargo` / `remove_cargo` return `false` instead |

## Travel-time matrix

At init, `travel_times` must be:

- non-empty,
- square (N rows of N entries),
- zero on the diagonal,
- positive everywhere else.

It doesn't need to be symmetric or satisfy the triangle inequality.

## Inventories

- An entry is removed when its quantity drops to zero, so inventories never hold `0`.
- Repeated manifest entries for the same `(location, destination)` merge into one entry and keep the
  position of the first.
