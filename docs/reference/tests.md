# `tests/`

[← docs index](../README.md)

```
tests/
├── cli.rs                       spawns the binary
├── golden.rs                    runs fixture pairs through Engine
└── golden/
    ├── base_example.in.jsonl    / .out.jsonl   (9 lines)  base sim
    ├── cargo_example.in.jsonl   / .out.jsonl   (21 lines) docks + cargo
    └── history_example.in.jsonl / .out.jsonl   (25 lines) cargo + get_state_at
```

## `cli.rs`

A `run(input: &[u8]) -> (Vec<Value>, bool)` helper pipes bytes into
`CARGO_BIN_EXE_fleet-sim` and parses each stdout line.

| Test | Checks |
|---|---|
| `invalid_utf8_line_gets_an_error_and_the_loop_keeps_going` | bad bytes → error line, next command still runs |
| `blank_and_crlf_lines_are_handled` | blank lines skipped, `\r\n` accepted |

## `golden.rs`

- `run_case(name)` reads both files, drops blank lines, asserts equal line counts, then compares
  each response as `serde_json::Value`.
- The `golden!` macro generates one `#[test]` per fixture name and records them in
  `REGISTERED`. Attributes like `#[ignore = "..."]` pass through.
- `all_fixtures_registered` scans `tests/golden/` and fails on any `.in.jsonl` that isn't listed.

## Fixtures

The three fixtures are the worked examples from the spec. `.in` holds one command per line, and
`.out` holds the expected response on the same line number.
