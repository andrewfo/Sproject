# `src/engine.rs`

[← docs index](../README.md)

All simulation rules. It's the largest file (~1000 lines, most of it unit tests).

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
| `Sim` | `config`, `committed`, `planned`, `history` (see [state structs](../codebase-tour/state-structs.md)) |
| `Sim::init` | validates params, builds tick-0 world, seeds committed/planned/history |
| `Sim::dispatch` | matches a `Command` to its handler, renders planned on success |
| `Sim::render_planned` | `render_world(&planned, cargo_enabled)` |
| `Sim::committed_vessel` | looks up a vessel in committed or returns `UnknownVessel` |
| `Sim::check_location` | range check or returns `LocationOutOfRange` |

## Handlers

| Handler | Status | Notes |
|---|---|---|
| `tick` | done | builds `next` from planned, lands arrivals, applies manifest, commits |
| `set_destination` | done | committed must be idle; same location cancels; `checked_add` for arrival |
| `dock`, `undock` | stub | returns `NotImplemented` |
| `load`, `unload`, `swap` | stub | TODO: one shared helper that works on a scratch planned copy and counts dock ops against throughput |
| `get_state_at` | stub | TODO: bounds-check against current tick, render `history[tick]` |

## Validation helpers

| Function | Checks |
|---|---|
| `validate_travel_times` | non-empty, square, zero diagonal, positive off-diagonal |
| `validate_dock_throughput` | length == N, all positive |
| `validate_manifest` | every location/destination in range, quantity > 0; returns `Err(String)` so callers can wrap it in `InvalidInit` or `InvalidManifest` |
| `apply_manifest` | adds entries to docks; fails on overflow (callers pass a scratch copy) |

## Tests (`mod tests`)

Grouped roughly by feature: init validation, init state, `set_destination`, tick and arrivals,
history, cargo init, tick manifests, strict field parsing. Shared helpers:

`init_line`, `std_init`, `initialized`, `assert_error`, `set_dest`, `planned_status`,
`committed_status`, `cargo_init_line`, `tick_with`, `cargo_initialized`, `assert_invalid_command`.
