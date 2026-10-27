# Error handling

[← docs index](../README.md)

## Rule: a failed command changes nothing

Every handler is written so that returning `Err` leaves `committed`, `planned`, and `history`
exactly as they were. Two techniques get there:

### 1. Validate everything first

Check all preconditions before the first write. `set_destination` is the model:

```rust
let vessel = self.committed_vessel(vessel_id)?;     // unknown vessel?
self.check_location(destination)?;                  // out of range?
let VesselStatus::Idle { .. } = vessel.status else { return Err(NotIdle) };
let arrives = tick.checked_add(travel)              // overflow?
    .ok_or(ArrivalOverflow { .. })?;
self.planned.vessels[vessel_id].status = status;    // only write, at the very end
```

### 2. Work on a scratch copy

When a failure can only be found partway through applying (manifest overflow), apply to a clone
and swap it in on success. `tick` builds `next` from `planned` and only assigns it at the end.
`init` builds the whole `Sim` locally and only stores it in `Engine` on `Ok`.

The planned cargo handlers (`load`/`unload`/`swap`) are meant to follow the same pattern:
apply to a scratch copy of planned, count operations at the dock, then commit or reject.

## Check order

When more than one thing is wrong, the error you get depends on check order. The current order:

- **Envelope:** parse → initialized? → handler.
- **`set_destination`:** unknown vessel → location range → idle → arrival overflow.
- **`init`:** travel matrix → throughput (or manifest-without-throughput) → each vessel (range,
  then duplicate id) → manifest entries → manifest overflow.
- **`tick`:** manifest in base mode → manifest entries → overflow.
- **transfers (`load`/`unload`/`swap`):** unknown vessel → location range → docked at the last
  tick and not undocking → load unit → vessel overflow → unload unit → dock overflow → throughput.

Unit tests pin some of these (for example `set_destination_checks_vessel_before_location`).

## Never panic on input

- Bad UTF-8, bad JSON, wrong types, unknown fields → `InvalidCommand`.
- Arithmetic on ticks and quantities uses `checked_add` or explicit guards.
- Indexing (`travel_times[from][to]`, `docks[loc]`) only happens after a range check.
- The one `unreachable!` (init inside `dispatch`) can't be reached because `Engine::handle`
  intercepts `Init` first.

The process only exits non-zero on a real stdin/stdout I/O error.
