# `src/main.rs`

[← docs index](../README.md) · [all reference pages](../README.md#pages)

The binary entry point. It's the I/O loop and nothing else (28 lines).

## What it does

1. Locks stdout and creates an `Engine`.
2. Reads stdin with `split(b'\n')`, so it gets raw bytes per line.
3. Strips a trailing `\r`.
4. Decodes UTF-8:
   - blank/whitespace-only → `continue` (no output)
   - valid → `engine.handle_line(&line)`
   - invalid → `render_response(Err(InvalidCommand("line is not valid UTF-8")))`
5. `writeln!`s the response.

Returns `io::Result<()>`, so only a real I/O failure makes the process exit with an error.

## Why bytes instead of `lines()`

`BufRead::lines()` yields `Err` on invalid UTF-8, and `?` would end the program. Reading bytes
lets one bad line get an error response while the loop keeps going. `tests/cli.rs` covers this.

## Uses

`fleet_sim::Engine`, `fleet_sim::error::SimError`, `fleet_sim::protocol::render_response`.
