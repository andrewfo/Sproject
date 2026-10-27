//! Fleet sim engine. Reads NDJSON commands, writes one JSON response per command.

pub mod engine;
pub mod error;
pub mod protocol;
pub mod state;

pub use engine::Engine;
