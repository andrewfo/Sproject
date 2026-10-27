# Responses

[← docs index](../README.md)

## Envelope

Every response is a single line:

```json
{"status":"ok","payload":<WorldState>}
{"status":"error","payload":{"message":"<text>"}}
```

Built by `protocol::render_response`. Messages are listed in [errors.md](errors.md).

## `WorldState`, base mode

```json
{
  "tick": 0,
  "vessels": {
    "v1": {"state": "transit", "from": 0, "to": 2, "arrives_at_tick": 7},
    "v2": {"state": "idle", "location": 2}
  }
}
```

| `state` | Other fields |
|---|---|
| `idle` | `location` |
| `docked` | `location` |
| `transit` | `from`, `to`, `arrives_at_tick` |

`vessels` is an object keyed by vessel id, in `init` order.

## `WorldState`, cargo mode

When `init` included `dock_throughput`, every vessel gets a `cargo` array and the world gets a
top-level `cargo` array of dock inventory:

```json
{
  "tick": 1,
  "vessels": {
    "v1": {"state": "docked", "location": 0, "cargo": [{"destination": 2, "quantity": 1}]}
  },
  "cargo": [
    {"location": 0, "destination": 2, "quantity": 1},
    {"location": 0, "destination": 1, "quantity": 1}
  ]
}
```

- Vessel `cargo`: `{destination, quantity}`, in first-loaded order.
- World `cargo`: docks flattened by location index, then by first-added destination order.
- Zero-quantity entries never appear.
- Arrays are `[]` when empty, never omitted in cargo mode.

## Which world is rendered

| Command | Payload |
|---|---|
| everything except `get_state_at` | planned world |
| `get_state_at` (planned) | committed world from history |
