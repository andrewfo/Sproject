# State structs

[← docs index](../README.md)

## Shape

```
Engine
└── sim: Option<Sim>          None until a successful init
    ├── config: Config        fixed at init
    │   ├── travel_times      N×N matrix
    │   └── dock_throughput   Option<Vec<u32>>; None = base mode (no cargo)
    ├── committed: World      what's true as of the last tick
    ├── planned: World        committed + changes queued since the last tick
    └── history: Vec<World>   history[t] = committed world at tick t
```

```
World
├── tick: u64
├── vessels: IndexMap<VesselId, Vessel>   init order preserved
│   └── Vessel { status: VesselStatus, cargo: Inventory }
└── docks: Vec<Inventory>                 one per location

VesselStatus = Idle { location }
             | Docked { location }
             | Transit { from, to, arrives_at_tick }

Inventory = IndexMap<Location (destination), u32 (quantity)>
```

## Committed vs planned

Commands issued between ticks don't change reality right away. They edit `planned`. A `tick`
turns `planned` into the new `committed` world.

```
         set_destination, (later: dock, load, ...)
                │ edit
                ▼
 committed ── clone at tick ──▶ planned ──┐
     ▲                                     │ tick: clone planned, advance tick,
     │                                     │ land arrivals, apply manifest
     └──────────── new committed ◀─────────┘
                        │
                        └──▶ pushed onto history
```

- **Precondition checks read committed.** "Is the vessel idle?" uses the committed status, so a
  vessel that was planned into transit this tick can still be re-planned. That's why calling
  `set_destination` twice before a tick means the second call wins.
- **Responses show planned.** Every successful command (except `get_state_at`) returns the planned
  world, so the caller sees the effect of what they just queued.
- **After a tick, committed == planned.**

## Tick

`Sim::tick` does this, in order:

1. Reject a non-empty `cargo_manifest` in base mode.
2. Validate every manifest entry (range, positive quantity).
3. Clone `planned` into `next`, increment `next.tick`.
4. Every vessel in `Transit` with `arrives_at_tick <= next.tick` becomes `Idle` at `to`.
5. Apply the manifest to `next.docks` (this can still fail on `u32` overflow).
6. Set `committed = planned = next`, and push `next` onto `history`.

If anything fails before step 6, the real state is untouched because all work happened on `next`.

A vessel arriving on tick *t* is already idle in the tick-*t* response and can be given a new
destination right away.

## Base mode vs cargo mode

`Config::cargo_enabled()` is just `dock_throughput.is_some()`. In base mode:

- `cargo_manifest` on `init` or `tick` is an error,
- `docks` are still allocated but always empty,
- rendered output has no `cargo` fields at all (vessel or world level).

## History

`history` starts with the tick-0 world from `init` and gets one entry per successful tick, so
`history.len() == committed.tick + 1`. The `get_state_at` query that reads it isn't wired up yet.
