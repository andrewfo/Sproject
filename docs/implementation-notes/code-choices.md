# Code choices

[← docs index](../README.md)

Each entry: what was chosen, what it buys, and what it costs.

## Library crate + thin binary

**Choice.** All logic is in `fleet_sim` (`lib.rs`), and `main.rs` only does I/O.
**Buys.** Tests drive `Engine::handle_line` in-process with plain strings. No subprocess needed
except for the few stdin edge cases in `tests/cli.rs`.
**Costs.** Nothing much. There's one extra file.

## `Option<Sim>` for the init state

**Choice.** `Engine { sim: Option<Sim> }`, with `Sim` only built by a successful `init`.
**Buys.** No half-initialized state. A failed init leaves `None`, and the match in
`Engine::handle` covers all four (initialized × is-init) cases.
**Costs.** Init is routed separately from `Sim::dispatch`, which has an `unreachable!` arm for it.

## Two full `World` copies (committed + planned)

**Choice.** Keep `planned` as a whole cloned world instead of a list of pending ops.
**Buys.** Rendering the response is just rendering `planned`. Re-planning overwrites in place
("second call wins"). The tick commits with a single assignment.
**Costs.** A clone per tick, plus another one into history. With small fleets this is cheap.

## History as `Vec<World>` snapshots

**Choice.** Store a full committed world per tick.
**Buys.** `get_state_at(t)` is an index lookup, with no replay and no event log to keep
consistent.
**Costs.** Memory grows linearly with ticks × fleet size. Structural sharing or diffs would cut
that, and it would only matter for very long runs.

## Rendering by hand with `json!`

**Choice.** `render_world` builds `serde_json::Value` manually instead of deriving `Serialize`.
**Buys.** The output shape (the `state` tag, flattened fields, conditional `cargo` keys, docks
flattened to a list) is spelled out in one place and matches the spec exactly.
**Costs.** Adding a field means touching the renderer by hand.

## `IndexMap` + `preserve_order`

**Choice.** Insertion-ordered maps for vessels and inventories, and order-preserving JSON objects.
**Buys.** Output order matches the spec examples byte-for-byte (vessels in init order, cargo in
first-seen order).
**Costs.** `shift_remove` on removal is O(n). Inventories are tiny, so that's fine.

## Strict parsing

**Choice.** `deny_unknown_fields` on every command and parameter struct.
**Buys.** A typo like `cargo_manfest` is an error instead of being silently ignored. A misspelled
tick field doesn't advance the clock.
**Costs.** Clients can't send extra metadata fields.

## Signed `tick` in `get_state_at`

**Choice.** Parse `tick` as `i128` even though ticks are `u64`.
**Buys.** `-1` gets a clear `tick -1 is negative` error instead of a generic serde type error, and
a tick bigger than `u64::MAX` gets the normal "in the future" error instead of a parse error.

## Error text lives in `Display`

**Choice.** `SimError` variants hold data, and `impl Display` produces the exact wire message.
**Buys.** One place to audit every message (see [protocol/errors.md](../protocol/errors.md)), and
tests can compare messages exactly.

## Validate, then mutate

See [error handling](error-handling.md).
