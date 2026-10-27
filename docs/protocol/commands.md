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
| [`dock`](#dock--undock) / [`undock`](#dock--undock) | not implemented | planned |
| [`load`](#load--unload--swap) / [`unload`](#load--unload--swap) / [`swap`](#load--unload--swap) | not implemented | planned |
| [`get_state_at`](#get_state_at) | not implemented | nothing |

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

- Calling it again before the next tick replaces the earlier plan.
- Setting the destination to the vessel's current location cancels a pending move.

## `get_state`

```json
{"command":"get_state","parameters":{}}
```

Returns the planned world. Changes nothing.

## `dock` / `undock`

```json
{"command":"dock","parameters":{"vessel_id":"v1"}}
```

Parsed, but currently returns `dock is not implemented yet` (same for `undock`).

## `load` / `unload` / `swap`

```json
{"command":"load","parameters":{"vessel_id":"v1","destination":2}}
{"command":"swap","parameters":{"vessel_id":"v1","load_destination":2,"unload_destination":1}}
```

Parsed, but currently return `<name> is not implemented yet`. Error variants for them already
exist: `NotDocked`, `InsufficientCargo`, `ThroughputExceeded`.

## `get_state_at`

```json
{"command":"get_state_at","parameters":{"tick":3}}
```

Parsed (`tick` is signed so negatives get a clean error), but currently returns
`get_state_at is not implemented yet`. It's meant to return the committed world at that tick.
`NegativeTick` and `TickInFuture` are already defined for it.
