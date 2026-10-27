# `src/engine.rs`

[← docs index](../README.md)

All simulation rules. It's the largest file (~1900 lines, most of it unit tests).

## Public API

```rust
pub struct Engine { /* sim: Option<Sim> */ }

impl Engine {
    pub fn new() -> Self;
    pub fn handle_line(&mut self, line: &str) -> String;
}
```

`handle_line` is parse → handle → render. It always returns exactly one JSON line.

## Private structure

| Item | Role |
|---|---|
| `Engine::handle` | routes on init state (see [lifecycle](../codebase-tour/command-lifecycle.md)) |
| `Sim` | `config`, `committed`, `planned`, `history`, `deliveries` (see [state structs](../codebase-tour/state-structs.md)) |
| `Sim::init` | validates params, builds tick-0 world, seeds committed/planned/history |
| `Sim::dispatch` | rejects dock/cargo commands in base mode, matches a `Command` to its handler, renders planned on success |
| `Sim::render_planned` | `render_world(&planned, cargo_enabled)` |
| `Sim::committed_vessel` | looks up a vessel in committed or returns `UnknownVessel` |
| `Sim::check_location` | range check or returns `LocationOutOfRange` |

## Handlers

| Handler | Status | Notes |
|---|---|---|
| `tick` | done | builds `next` from planned, lands arrivals, applies manifest, commits |
| `set_destination` | done | committed must be idle; same location cancels; `checked_add` for arrival |
| `dock` | done | committed idle or docked; overrides a pending move, cancels a pending undock |
| `undock` | done | committed must be docked; plans idle, keeps pending cargo ops |
| `transfer` | done | shared by load/unload/swap: checks docked with no pending undock, applies the load then the unload to a scratch copy of planned, records deliveries, checks `dock_ops` against throughput, keeps the copy only if all of it passed |
| `load`, `unload`, `swap` | done | thin wrappers over `transfer` |
| `get_state_at` | done | negative → `NegativeTick`, past current → `TickInFuture`, else renders `history[tick]` |

## Validation helpers

| Function | Checks |
|---|---|
| `validate_travel_times` | non-empty, square, zero diagonal, positive off-diagonal |
| `validate_dock_throughput` | length == N, all positive |
| `validate_manifest` | every location/destination in range, quantity > 0; returns `Err(String)` so callers can wrap it in `InvalidInit` or `InvalidManifest` |
| `apply_manifest` | adds entries to docks; fails on overflow (callers pass a scratch copy) |
| `dock_ops` | ops at one dock: per vessel docked there at the last tick, `max(loads, unloads)` from the committed → planned cargo diff, with that vessel's `deliveries` added back as unloads |

## Tests (`mod tests`)

Grouped roughly by feature: init validation, init state, `set_destination`, tick and arrivals,
history, cargo init, tick manifests, dock/undock, `dock_ops` on hand-built worlds, `transfer`,
deliveries, load/unload/swap, `get_state_at`, strict field parsing. Shared helpers include:

`init_line`, `std_init`, `initialized`, `assert_error`, `set_dest`, `planned_status`,
`committed_status`, `cargo_init_line`, `tick_with`, `cargo_initialized`, `dock`, `undock`,
`docked_engine`, `vessel`, `world`, `stocked_engine`, `own_dest_engine`, `sim`, `load`,
`unload`, `swap`, `assert_invalid_command`.
