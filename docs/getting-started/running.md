# Running

[← docs index](../README.md)

## Requirements

- Rust with edition 2024 support (1.85 or newer)
- Dependencies are pulled by Cargo: `serde`, `serde_json` (with `preserve_order`), `indexmap`

## Build and run

```sh
cargo build --release
cargo run --release < tests/golden/base_example.in.jsonl
```

The binary is called `fleet-sim`. It reads newline-delimited JSON (NDJSON) commands from stdin and
writes exactly one JSON response line to stdout per command. It exits when stdin closes.

## Interactive use

```sh
cargo run
```

Then paste commands one per line:

```json
{"command":"init","parameters":{"travel_times":[[0,4],[4,0]],"vessels":[{"id":"v1","location":0}]}}
{"command":"set_destination","parameters":{"vessel_id":"v1","destination":1}}
{"command":"tick","parameters":{}}
```

Each response is flushed as soon as it's written, so you see it right away.

## Input handling

- Blank and whitespace-only lines are skipped and get no response.
- A trailing `\r` is stripped, so CRLF input works.
- A line that isn't valid UTF-8 gets an `invalid command` error and the loop keeps going.
- Anything else that fails to parse gets an `invalid command` error with serde's reason.

See [protocol/commands.md](../protocol/commands.md) for the full command list.
