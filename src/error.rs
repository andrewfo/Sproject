use std::fmt;

use crate::state::{Location, Tick};

/// Every error the engine can report. `Display` produces the exact message sent to stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimError {
    InvalidCommand(String),
    NotInitialized,
    AlreadyInitialized,
    InvalidInit(String),
    UnknownVessel(String),
    LocationOutOfRange(Location),
    NotIdle(String),
    ArrivalOverflow {
        from: Location,
        to: Location,
    },
    NotDocked(String),
    InsufficientCargo {
        what: String,
        destination: Location,
    },
    ThroughputExceeded {
        ops: u32,
        cap: u32,
        location: Location,
    },
    TickInFuture {
        tick: i64,
        current: Tick,
    },
    NegativeTick(i64),
    NotImplemented(&'static str),
}

impl fmt::Display for SimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimError::InvalidCommand(why) => write!(f, "invalid command: {why}"),
            SimError::NotInitialized => write!(f, "simulation not initialized; send init first"),
            SimError::AlreadyInitialized => write!(f, "simulation already initialized"),
            SimError::InvalidInit(why) => write!(f, "invalid init: {why}"),
            SimError::UnknownVessel(id) => write!(f, "unknown vessel {id}"),
            SimError::LocationOutOfRange(loc) => write!(f, "location {loc} is out of range"),
            SimError::NotIdle(id) => write!(f, "vessel {id} is not idle"),
            SimError::ArrivalOverflow { from, to } => {
                write!(f, "travel from {from} to {to} overflows the tick counter")
            }
            SimError::NotDocked(id) => write!(f, "vessel {id} is not docked"),
            SimError::InsufficientCargo { what, destination } => {
                write!(f, "{what} has no cargo for destination {destination}")
            }
            SimError::ThroughputExceeded { ops, cap, location } => write!(
                f,
                "transfer of {ops} operations exceeds dock throughput of {cap} at location {location}"
            ),
            SimError::TickInFuture { tick, current } => {
                write!(f, "tick {tick} is in the future (current tick: {current})")
            }
            SimError::NegativeTick(tick) => write!(f, "tick {tick} is negative"),
            SimError::NotImplemented(cmd) => write!(f, "{cmd} is not implemented yet"),
        }
    }
}

impl std::error::Error for SimError {}
