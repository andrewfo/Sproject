# Testing

[← docs index](../README.md)

```sh
cargo test                  # unit, CLI, and golden tests
cargo clippy --all-targets  # lint
```

## Three layers

| Layer | Where | What it covers |
|---|---|---|
| Unit | `src/engine.rs` (`mod tests`) | Engine rules, driven through `Engine::handle_line` with JSON strings |
| Golden | `tests/golden.rs` + `tests/golden/*.jsonl` | Full command streams from the spec, checked line by line |
| CLI | `tests/cli.rs` | The real binary: UTF-8 errors, blank lines, CRLF, byte-exact golden output |

### Unit tests

They live next to the engine and go through the public line API, so they exercise parsing,
dispatch, and rendering together. Small helpers (`init_line`, `std_init`, `initialized`,
`assert_error`, `set_dest`, `tick_with`, ...) keep each test short.

### Golden tests

Each `NAME.in.jsonl` is fed line by line into a fresh `Engine`. Each response is compared to the
same line of `NAME.out.jsonl` as parsed JSON, so object key order doesn't matter but array order
does. A failure prints the line number, the command, and both values.

| Case | Covers |
|---|---|
| `base_example` | the spec's base example |
| `cargo_example` | the spec's cargo example |
| `history_example` | the cargo example plus its `get_state_at` queries |
| `errors_before_init` | commands before `init`, bad JSON, unknown command, double `init` |
| `bad_init` | each `init` validation error, then a retry that works |
| `throughput` | two vessels sharing a dock's throughput, swaps, load-then-unload netting to zero |
| `dock_state_machine` | dock/undock/set_destination overriding and cancelling each other |
| `delivery` | carrying cargo to its destination and unloading it out of the sim |
| `arrive_and_recommand` | arriving on a tick and getting a new destination that same tick |
| `history_bounds` | `get_state_at` bounds, and pending changes not leaking into history |

`all_fixtures_registered` fails if a `.in.jsonl` file exists without an entry in the `golden!`
list. See [contributing/adding-a-golden-test.md](../contributing/adding-a-golden-test.md).

### CLI tests

These spawn the compiled binary (`CARGO_BIN_EXE_fleet-sim`) because stdin decoding lives in
`main.rs`, which the engine tests can't reach. One of them also diffs raw stdout against each
golden `.out.jsonl` byte for byte, since the golden tests themselves ignore key order.
