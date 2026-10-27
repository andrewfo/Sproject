# `src/error.rs`

[← docs index](../README.md)

Defines `SimError`, the single error type for the whole crate.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimError { /* 15 variants */ }

impl fmt::Display for SimError { /* exact wire message */ }
impl std::error::Error for SimError {}
```

- Variants carry data (ids, locations, ticks), not preformatted strings, except the
  `InvalidCommand` / `InvalidInit` / `InvalidManifest` reason strings.
- `Display` is the only place message text lives. `render_response` calls `err.to_string()`.
- `PartialEq` lets tests compare errors directly if needed.

The full list of variants and their messages is in [protocol/errors.md](../protocol/errors.md).
