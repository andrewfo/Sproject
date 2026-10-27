# fleet-sim docs

Start here. Each folder covers one topic, and every page links back to this index.

| Folder | Read it when you want to... |
|---|---|
| [getting-started/](getting-started/) | build, run, and test the sim |
| [codebase-tour/](codebase-tour/) | see how the pieces fit and how a command moves through them |
| [implementation-notes/](implementation-notes/) | check what the code guarantees and why it is written that way |
| [protocol/](protocol/) | look up a command, the response shape, or an error message |
| [reference/](reference/) | read a per-file breakdown of `src/` and `tests/` |
| [contributing/](contributing/) | add a command, a golden test, or follow code conventions |

## Pages

- **Getting started**
  - [Running](getting-started/running.md)
  - [Testing](getting-started/testing.md)
- **Codebase tour**
  - [Module map](codebase-tour/module-map.md): modules and how they depend on each other
  - [Command lifecycle](codebase-tour/command-lifecycle.md): one line from stdin to stdout
  - [State structs](codebase-tour/state-structs.md): committed, planned, history
- **Implementation notes**
  - [Invariants](implementation-notes/invariants.md): what always holds
  - [Code choices](implementation-notes/code-choices.md): why the code is shaped the way it is
  - [Error handling](implementation-notes/error-handling.md): validate first, mutate last
- **Protocol**
  - [Commands](protocol/commands.md)
  - [Responses](protocol/responses.md): the envelope and `WorldState`
  - [Errors](protocol/errors.md): every error message
- **Reference**
  - [src/main.rs](reference/main.md)
  - [src/lib.rs](reference/lib.md)
  - [src/protocol.rs](reference/protocol.md)
  - [src/state.rs](reference/state.md)
  - [src/engine.rs](reference/engine.md)
  - [src/error.rs](reference/error.md)
  - [tests/](reference/tests.md)
- **Contributing**
  - [Adding a command](contributing/adding-a-command.md)
  - [Adding a golden test](contributing/adding-a-golden-test.md)
  - [Conventions](contributing/conventions.md)

## Status at a glance

| Feature | Commands | Status |
|---|---|---|
| Base sim | `init`, `tick`, `set_destination`, `get_state` | done |
| Cargo & docks | `dock`, `undock`, `load`, `unload`, `swap`, cargo manifests | manifests done; vessel commands return "not implemented yet" |
| History | `get_state_at` | history is recorded every tick; the query returns "not implemented yet" |
