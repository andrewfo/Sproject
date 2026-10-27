# Testing

[← docs index](../README.md)

```sh
cargo test                       # unit + CLI tests, plus golden cases that aren't ignored
cargo test -- --include-ignored  # also runs golden cases marked #[ignore]
cargo clippy --all-targets       # lint
```

## Three layers

| Layer | Where | What it covers |
|---|---|---|
| Unit | `src/engine.rs` (`mod tests`) | Engine rules, driven through `Engine::handle_line` with JSON strings |
| Golden | `tests/golden.rs` + `tests/golden/*.jsonl` | Full command streams from the spec, checked line by line |
| CLI | `tests/cli.rs` | The real binary: UTF-8 errors, blank lines, CRLF |

### Unit tests

They live next to the engine and go through the public line API, so they exercise parsing,
dispatch, and rendering together. Small helpers (`init_line`, `std_init`, `initialized`,
`assert_error`, `set_dest`, `tick_with`, ...) keep each test short.

### Golden tests

Each `NAME.in.jsonl` is fed line by line into a fresh `Engine`. Each response is compared to the
same line of `NAME.out.jsonl` as parsed JSON, so object key order doesn't matter but array order
does. A failure prints the line number, the command, and both values.

| Case | Status |
|---|---|
| `base_example` | runs |
| `cargo_example` | `#[ignore]` until dock/load/unload/swap land |
| `history_example` | `#[ignore]` until cargo commands and `get_state_at` land |

`all_fixtures_registered` fails if a `.in.jsonl` file exists without an entry in the `golden!`
list. See [contributing/adding-a-golden-test.md](../contributing/adding-a-golden-test.md).

### CLI tests

These spawn the compiled binary (`CARGO_BIN_EXE_fleet-sim`) because stdin decoding lives in
`main.rs`, which the engine tests can't reach.
