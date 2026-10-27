# Errors

[← docs index](../README.md)

Every error is `{"status":"error","payload":{"message":"..."}}`. The text comes from
`impl Display for SimError` in `src/error.rs`. Placeholders are in `<angle brackets>`.

| Variant | Message | Raised by |
|---|---|---|
| `InvalidCommand` | `invalid command: <reason>` | parse failure, unknown field, bad UTF-8 |
| `NotInitialized` | `simulation not initialized; send init first` | any command before `init` |
| `AlreadyInitialized` | `simulation already initialized` | second `init` |
| `InvalidInit` | `invalid init: <reason>` | `init` validation |
| `InvalidManifest` | `invalid manifest: <reason>` | `tick` manifest validation |
| `UnknownVessel` | `unknown vessel <id>` | vessel commands |
| `LocationOutOfRange` | `location <loc> is out of range (0 to <n-1>)` | `set_destination`, cargo commands |
| `NotIdle` | `vessel <id> is not idle` | `set_destination` on a non-idle committed vessel, `dock` on a vessel in transit |
| `ArrivalOverflow` | `travel from <from> to <to> overflows the tick counter` | `set_destination` |
| `NotDocked` | `vessel <id> is not docked` | `undock` on a vessel not docked at the last tick; `load`/`unload`/`swap` on one that isn't docked or has an undock pending |
| `InsufficientCargo` | `<what> has no cargo for destination <dest>` | `load`/`unload`/`swap` |
| `CargoOverflow` | `<what> would overflow its cargo for destination <dest>` | `load`/`unload`/`swap` |
| `ThroughputExceeded` | `transfer of <ops> operations exceeds dock throughput of <cap> at location <loc>` | `load`/`unload`/`swap` |
| `TickInFuture` | `tick <t> is in the future (current tick: <now>)` | `get_state_at` |
| `NegativeTick` | `tick <t> is negative` | `get_state_at` |
| `CargoDisabled` | `<command> requires dock_throughput` | `dock`/`undock`/`load`/`unload`/`swap` in base mode |

`<what>` is `vessel <id>` or `dock at location <loc>`.

## `<reason>` strings

### `invalid init: ...`

- `travel_times must have at least one location`
- `travel_times must be square: row <i> has <k> entries, expected <n>`
- `travel_times[<i>][<j>] must be 0` (diagonal)
- `travel_times[<i>][<j>] must be positive` (off-diagonal)
- `dock_throughput has <k> entries, expected <n>`
- `dock_throughput[<i>] must be positive`
- `cargo_manifest requires dock_throughput`
- `vessel <id> location <loc> is out of range (0 to <n-1>)`
- `duplicate vessel id <id>`
- any manifest reason below

### `invalid manifest: ...` (also used inside `invalid init`)

- `cargo_manifest requires dock_throughput` (tick in base mode)
- `cargo_manifest[<i>] location <loc> is out of range (0 to <n-1>)`
- `cargo_manifest[<i>] destination <loc> is out of range (0 to <n-1>)`
- `cargo_manifest[<i>] quantity must be positive`
- `cargo_manifest[<i>] overflows the quantity for destination <d> at location <l>`

### `invalid command: ...`

- `line is not valid UTF-8`
- otherwise serde_json's own message, e.g. `unknown variant`, `missing field`,
  `unknown field \`cargo_manfest\``, `invalid type`.
