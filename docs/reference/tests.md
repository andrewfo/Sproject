# `tests/`

[← docs index](../README.md)

```
tests/
├── cli.rs                       spawns the binary
├── golden.rs                    runs fixture pairs through Engine
└── golden/
    ├── base_example.in.jsonl    / .out.jsonl   (9 lines)  base sim
    ├── cargo_example.in.jsonl   / .out.jsonl   (21 lines) docks + cargo
    ├── history_example.in.jsonl / .out.jsonl   (25 lines) cargo + get_state_at
    └── ...                                     edge cases, see below
```

## `cli.rs`

`run_raw(input: &[u8]) -> (String, bool)` pipes bytes into `CARGO_BIN_EXE_fleet-sim` and
returns raw stdout. `run` wraps it and parses each line as JSON.

| Test | Checks |
|---|---|
| `invalid_utf8_line_gets_an_error_and_the_loop_keeps_going` | bad bytes → error line, next command still runs |
| `blank_and_crlf_lines_are_handled` | blank lines skipped, `\r\n` accepted |
| `golden_outputs_match_byte_for_byte` | binary output for every fixture equals its `.out.jsonl` exactly |

## `golden.rs`

- `run_case(name)` reads both files, drops blank lines, asserts equal line counts, then compares
  each response as `serde_json::Value`.
- The `golden!` macro generates one `#[test]` per fixture name and records them in
  `REGISTERED`. Attributes like `#[ignore = "..."]` pass through.
- `all_fixtures_registered` scans `tests/golden/` and fails on any `.in.jsonl` that isn't listed.

## Fixtures

`base_example`, `cargo_example` and `history_example` are the worked examples from the spec. The
rest are edge cases, one theme per file:

| Case | Covers |
|---|---|
| `errors_before_init` | commands before `init`, bad JSON, unknown command, double `init` |
| `bad_init` | each `init` validation error, then a retry that works |
| `throughput` | two vessels sharing a dock's throughput, swaps, load-then-unload netting to zero |
| `dock_state_machine` | dock/undock/set_destination overriding and cancelling each other |
| `delivery` | carrying cargo to its destination and unloading it out of the sim |
| `arrive_and_recommand` | arriving on a tick and getting a new destination that same tick |
| `history_bounds` | `get_state_at` bounds, and pending changes not leaking into history |

`.in` holds one command per line, and `.out` holds the expected response on the same line number.
