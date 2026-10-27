# Commands

[← docs index](../README.md)

Every input line is one JSON object:

```json
{"command": "<name>", "parameters": { ... }}
```

`parameters` is required, even when it's empty (`{}`). Unknown commands and unknown fields are
rejected. Types: `Location` is a non-negative integer index into `travel_times`. Quantities
are `u32`.

| Command | Status | Mutates |
|---|---|---|
| [`init`](#init) | done | creates the sim |
| [`tick`](#tick) | done | committed, planned, history |
| [`set_destination`](#set_destination) | done | planned |
| [`get_state`](#get_state) | done | nothing |
| [`dock`](#dock--undock) / [`undock`](#dock--undock) | done | planned |
| [`load`](#load--unload--swap) / [`unload`](#load--unload--swap) / [`swap`](#load--unload--swap) | done | planned |
| [`get_state_at`](#get_state_at) | done | nothing |

The five dock and cargo commands need cargo mode (`dock_throughput` on `init`). In base mode they
fail with `<name> requires dock_throughput`.

---

## `init`

```json
{"command":"init","parameters":{
  "travel_times":[[0,4,7],[4,0,3],[7,3,0]],
  "vessels":[{"id":"v1","location":0},{"id":"v2","location":2}],
  "dock_throughput":[2,4,1],
  "cargo_manifest":[{"location":0,"destination":2,"quantity":1}]
}}
```

| Field | Required | Rules |
|---|---|---|
| `travel_times` | yes | non-empty, square, `0` on diagonal, positive off-diagonal |
| `vessels` | yes | may be empty; `id` unique; `location` in range |
| `dock_throughput` | no | if present, enables cargo mode; one positive entry per location |
| `cargo_manifest` | no | needs `dock_throughput`; every location/destination in range; quantity > 0 |

Only allowed once. Returns the tick-0 world. All vessels start `idle`.

## `tick`

```json
{"command":"tick","parameters":{}}
{"command":"tick","parameters":{"cargo_manifest":[{"location":0,"destination":1,"quantity":3}]}}
```

Advances one tick and commits the plan. Vessels whose `arrives_at_tick` is at or before the new
tick become `idle` at their destination. Then the optional manifest adds cargo to the docks.
A bad manifest rejects the whole tick, and the clock doesn't move.

## `set_destination`

```json
{"command":"set_destination","parameters":{"vessel_id":"v1","destination":2}}
```

The vessel must be **idle in committed state**. It's planned into `transit` with
`arrives_at_tick = committed.tick + travel_times[from][destination]`.

- Calling it again before the next tick replaces the earlier plan, including a pending `dock`.
- Setting the destination to the vessel's current location cancels a pending move.
- A vessel docked at the last tick is not idle, even with an undock pending. It can leave on the
  tick after the undock commits.

## `get_state`

```json
{"command":"get_state","parameters":{}}
```

Returns the planned world. Changes nothing.

## `dock` / `undock`

```json
{"command":"dock","parameters":{"vessel_id":"v1"}}
```

`dock` needs the vessel to be idle or docked in committed state:

- committed idle → planned `docked` at its location, replacing any pending `set_destination`,
- committed docked with an undock pending → cancels the undock,
- already docked → no-op.

`undock` needs the vessel to be docked in committed state, and plans it back to `idle`. Undocking
twice is a no-op. Cargo ops queued earlier in the tick stay. A vessel that's only docked in the
plan can't undock (`vessel <id> is not docked`); cancel the dock with `set_destination` to its
current location instead.

## `load` / `unload` / `swap`

```json
{"command":"load","parameters":{"vessel_id":"v1","destination":2}}
{"command":"swap","parameters":{"vessel_id":"v1","load_destination":2,"unload_destination":1}}
```

`load` moves one unit for `destination` from the dock onto the vessel. `unload` moves one unit
the other way, or delivers it (removes it from the sim) if `destination` is the dock's own
location. `swap` is a load and an unload in one command, and it either fully applies or not at all.
Both sides are checked against the state before the swap, so the vessel has to already hold the
unit it unloads, and it can't load back the unit it just unloaded.

- The vessel must be docked in committed state and have no pending undock.
- The unit has to exist in planned state, so two loads can't take the same unit.
- **Throughput** is counted from the diff between each docked vessel's committed and planned
  cargo, not by counting commands. Per vessel, ops = `max(units loaded, units unloaded)`, so a
  swap is one op and loading then unloading the same unit is zero. Delivered units count as
  unloads. The sum over vessels at the dock must stay within `dock_throughput[location]`, or
  the command fails with `transfer of <n> operations exceeds dock throughput of <cap> at location <loc>`.

## `get_state_at`

```json
{"command":"get_state_at","parameters":{"tick":3}}
```

Returns the committed world as of that tick, from history. Pending planned changes never show
up. `tick` is parsed as a wide signed integer, so a negative tick gets `tick <t> is negative` and
any tick past the current one, however large, gets `tick <t> is in the future (current tick: <now>)`.
