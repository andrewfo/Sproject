# Command lifecycle

[← docs index](../README.md)

What happens to one line of input, start to finish.

```
stdin bytes
   │  split on b'\n', strip trailing b'\r'
   ▼
main.rs ── not UTF-8? ──▶ render_response(Err(InvalidCommand)) ──┐
   │      blank? ──▶ skip (no output)                             │
   ▼                                                               │
Engine::handle_line(&str)                                          │
   │                                                               │
   ├─ protocol::parse_command ── serde error ──▶ InvalidCommand ──┤
   ▼                                                               │
Engine::handle(Command)                                            │
   │  (None, Init)     → Sim::init → render planned                │
   │  (Some, Init)     → AlreadyInitialized                        │
   │  (None, _)        → NotInitialized                            │
   │  (Some, cmd)      → Sim::dispatch                             │
   ▼                                                               │
Sim::dispatch                                                      │
   │  run handler → Ok(()) or Err(SimError)                        │
   │  on Ok: render planned world (get_state_at will render history)
   ▼                                                               │
protocol::render_response(Result<Value, SimError>) ◀───────────────┘
   │
   ▼
writeln!(stdout)  →  one JSON line
```

## Steps

1. **Read.** `main.rs` reads raw bytes with `split(b'\n')` instead of `lines()`, so invalid UTF-8
   on one line produces an error response rather than an I/O error that ends the loop.
2. **Parse.** `protocol::parse_command` uses serde's adjacently tagged enum
   (`command` + `parameters`). Unknown commands, missing fields, wrong types, and unknown fields
   all become `SimError::InvalidCommand(<serde message>)`.
3. **Route.** `Engine::handle` checks the init state. `Engine` holds `Option<Sim>`, so "not
   initialized" is a type-level state, not a flag.
4. **Execute.** `Sim::dispatch` calls the handler. Handlers return `Result<(), SimError>` and
   follow validate-then-mutate (see [error handling](../implementation-notes/error-handling.md)).
5. **Render.** On success, the payload is the **planned** world rendered as `WorldState`.
   `get_state_at` is the one exception: it returns its own payload (a historical committed world).
6. **Write.** `render_response` wraps it in `{"status": ..., "payload": ...}` and serializes it to a
   single line.

Every input line that isn't blank produces exactly one output line, success or failure.
