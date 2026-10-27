# `src/lib.rs`

[← docs index](../README.md)

The crate root for the `fleet_sim` library.

```rust
pub mod engine;
pub mod error;
pub mod protocol;
pub mod state;

pub use engine::Engine;
```

All modules are public so integration tests and the binary can reach them. `Engine` is re-exported
because it's the only thing most callers need:

```rust
let mut engine = fleet_sim::Engine::new();
let response: String = engine.handle_line(r#"{"command":"get_state","parameters":{}}"#);
```
