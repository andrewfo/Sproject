# Module map

[← docs index](../README.md)

fleet-sim is a single-threaded, synchronous program. It has no I/O beyond stdin and stdout and
keeps all state in memory for the life of the process.

## Modules

```
            ┌────────────┐
 stdin ───▶ │  main.rs   │ ───▶ stdout
            └─────┬──────┘
                  │ handle_line(&str) -> String
            ┌─────▼──────┐
            │ engine.rs  │  Engine / Sim: rules, validation, ticks, history
            └──┬──────┬──┘
   parse/render│      │ data types
        ┌──────▼──┐ ┌─▼────────┐
        │protocol │ │ state.rs │
        │  .rs    │ └──────────┘
        └────┬────┘
             │
        ┌────▼────┐
        │error.rs │  SimError + message text (used by all of the above)
        └─────────┘
```

| Module | Owns | Knows nothing about |
|---|---|---|
| `main.rs` | byte-level stdin reading, line splitting, writing responses | sim rules |
| `engine.rs` | all simulation rules and validation; the `Engine` entry point | stdin/stdout |
| `protocol.rs` | JSON command types, the response envelope, `WorldState` rendering | rules |
| `state.rs` | plain data: `World`, `Vessel`, `VesselStatus`, `Config`, cargo helpers | JSON, rules |
| `error.rs` | `SimError` and the exact text of every error message | everything else |

`lib.rs` exposes these as a library crate (`fleet_sim`), and `main.rs` is a thin binary on top.
That split lets tests drive `Engine` directly without spawning a process.

## Dependency direction

```
main ──▶ engine ──▶ protocol ──▶ state
   │        │           │
   └────────┴───────────┴──────▶ error
```

`state` sits at the bottom with no crate-internal dependencies. `error` only borrows the
`Location` and `Tick` aliases from it. Nothing imports `engine` except `main`, `lib`, and tests.

## External crates

| Crate | Used for |
|---|---|
| `serde` | deriving `Deserialize` on commands, with `deny_unknown_fields` |
| `serde_json` | parsing commands and writing responses |
| `indexmap` | insertion-ordered maps for vessels and cargo, so output order matches the spec |

## Further reading

- [Command lifecycle](command-lifecycle.md)
- [State structs](state-structs.md)
- [Invariants](../implementation-notes/invariants.md)
