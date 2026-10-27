# Adding a command

[← docs index](../README.md)

A checklist that follows the existing pattern.

1. **Protocol.** Add a variant to `Command` in `src/protocol.rs`. Field names are the JSON
   parameter names. The variant name becomes snake_case automatically. Use `{}` for a command
   with no parameters.
2. **Errors.** Add any new `SimError` variants in `src/error.rs`, along with their `Display`
   arm. Put data in the variant and the wording in `Display`.
3. **Dispatch.** Add an arm in `Sim::dispatch` (`src/engine.rs`). Return `Ok(())` to render
   planned state, or `return` early with your own payload if the command renders something else.
4. **Handler.** Write `fn your_command(&mut self, ...) -> Result<(), SimError>`:
   - look the vessel up with `committed_vessel` and range-check with `check_location`,
   - run every check before the first write, or mutate a clone and swap it in,
   - read committed for status checks and planned for cargo checks.
5. **Unit tests.** Add tests in `mod tests` that drive `handle_line`: the happy path, each error,
   and "error leaves state unchanged".
6. **Docs.** Update [protocol/commands.md](../protocol/commands.md),
   [protocol/errors.md](../protocol/errors.md), and the status table in the
   [docs index](../README.md).
7. **Golden.** If a spec fixture covers it, remove its `#[ignore]` in `tests/golden.rs`.

Run `cargo test -- --include-ignored && cargo clippy --all-targets` before committing.
