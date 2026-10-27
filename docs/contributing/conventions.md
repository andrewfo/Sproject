# Conventions

[← docs index](../README.md)

## Code

- **Module boundaries.** Rules go in `engine.rs`, JSON goes in `protocol.rs`, data types go in
  `state.rs`, message text goes in `error.rs`. Don't put `serde_json` in `state.rs` or rules in
  `protocol.rs`.
- **Errors.** Return `Result<_, SimError>`. Never panic on input. Use `checked_*` arithmetic for
  ticks and quantities.
- **Mutation.** Validate everything first, or work on a clone. A failing command must leave state
  unchanged.
- **Ordering.** Use `IndexMap` wherever output order matters. Use `shift_remove`, not
  `swap_remove`, to keep order.
- **`#[must_use]`** on helpers that signal failure with `bool`.
- **Comments.** Short and plain. Module `//!` docs say what the file is for. `///` goes on
  non-obvious functions, and it explains *why* or the edge case, not what the signature already says.
- **Lint.** `cargo clippy --all-targets` should be clean. `cargo fmt` formatting.

## Tests

- Test through `Engine::handle_line` with JSON strings, not private functions.
- Test names describe behavior: `set_destination_second_call_wins`,
  `bad_tick_manifest_leaves_tick_and_state_alone`.
- Every error path gets a test, and so does the "state is unchanged afterwards" check where it matters.
- Compare error messages exactly. They're part of the protocol.

## Docs

- When you change a command or error message, update `docs/protocol/` in the same change.
- Keep the status table in `docs/README.md` current.
