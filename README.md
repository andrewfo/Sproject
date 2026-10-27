# fleet-sim

A discrete-time simulation engine for a fleet of vessels. It reads newline-delimited JSON commands
on stdin and writes one JSON response per command to stdout.

## Run

```sh
cargo run --release < tests/golden/base_example.in.jsonl
```

You can also run it interactively: `cargo run`, then type commands one per line.

## Test

```sh
cargo test   # unit, CLI, and golden spec examples
```

Golden cases are in `tests/golden/`. Each `NAME.in.jsonl` is a command stream, and the matching
`NAME.out.jsonl` holds the expected response for each line. To add a case, drop in a new pair
and register its name in the `golden!` list in `tests/golden.rs`.

## Docs

Full docs are in [`docs/`](docs/README.md). They cover a codebase tour,
implementation notes, the protocol reference, per-file notes, and contributing guides.

## Layout

| Path | What |
|---|---|
| `src/main.rs` | stdin/stdout loop |
| `src/protocol.rs` | command parsing, response rendering |
| `src/state.rs` | world state data types |
| `src/engine.rs` | simulation rules (committed vs planned state, ticks, cargo, history) |
| `src/error.rs` | error types and messages |
