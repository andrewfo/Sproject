//! Discrete-time fleet simulation engine driven by NDJSON commands.

pub mod engine;
pub mod error;
pub mod protocol;
pub mod state;

pub use engine::Engine;
