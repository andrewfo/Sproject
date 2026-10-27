# `src/protocol.rs`

[← docs index](../README.md)

JSON in and out. It has no sim rules.

## Types

| Item | Kind | Notes |
|---|---|---|
| `Command` | enum | `#[serde(tag = "command", content = "parameters", rename_all = "snake_case", deny_unknown_fields)]`. One variant per command. |
| `InitParams` | struct | `travel_times`, `vessels`, optional `dock_throughput`, `cargo_manifest` (defaults to empty) |
| `VesselSpec` | struct | `id`, `location` |
| `TickParams` | struct | `cargo_manifest` (defaults to empty) |
| `ManifestEntry` | struct | `location`, `destination`, `quantity: u32` |

All structs use `deny_unknown_fields`. `GetState {}` is an empty struct variant so it still
requires `"parameters": {}`. `GetStateAt.tick` is `i64` on purpose (see
[code choices](../implementation-notes/code-choices.md#signed-tick-in-get_state_at)).

Travel times parse as `u64`, so negative values fail at parse time with `InvalidCommand`.

## Functions

### `parse_command(line: &str) -> Result<Command, SimError>`

`serde_json::from_str`, with any error mapped to `SimError::InvalidCommand(e.to_string())`.

### `render_response(result: Result<Value, SimError>) -> String`

Wraps the result in `{"status":"ok","payload":...}` or
`{"status":"error","payload":{"message":...}}` and returns one serialized line.

### `render_world(world: &World, cargo_enabled: bool) -> Value`

Builds the `WorldState` JSON by hand with `json!`:

- each vessel → `{"state": ..., <status fields>}`, plus a `cargo` array when cargo mode is on
- docks → flattened `[{location, destination, quantity}]` in location order, emitted only in
  cargo mode

See [protocol/responses.md](../protocol/responses.md) for the output shape.
