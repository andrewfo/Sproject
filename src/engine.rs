//! Simulation rules: committed vs planned state, ticks, cargo ops, history.
//!
//! Invariants:
//! - `planned == committed` right after every tick.
//! - Status checks (idle/docked) validate against `committed`; inventory checks validate against `planned`.
//! - Every handler validates fully before mutating, so a rejected command changes nothing.

use serde_json::Value;

use crate::error::SimError;
use crate::protocol::{self, Command, InitParams, TickParams};
use crate::state::{Config, Location, World};

#[derive(Default)]
pub struct Engine {
    sim: Option<Sim>,
}

struct Sim {
    config: Config,
    committed: World,
    planned: World,
    /// `history[t]` is the committed state at tick `t`.
    history: Vec<World>,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses one input line, runs it, and returns the response line (always a single JSON object).
    pub fn handle_line(&mut self, line: &str) -> String {
        let result = protocol::parse_command(line).and_then(|cmd| self.handle(cmd));
        let response = match result {
            Ok(payload) => protocol::ok_response(payload),
            Err(err) => protocol::error_response(&err),
        };
        response.to_string()
    }

    /// Runs a parsed command and returns the WorldState payload.
    pub fn handle(&mut self, cmd: Command) -> Result<Value, SimError> {
        match (&mut self.sim, cmd) {
            (None, Command::Init(params)) => {
                let sim = Sim::init(params)?;
                let payload = sim.render_planned();
                self.sim = Some(sim);
                Ok(payload)
            }
            (Some(_), Command::Init(_)) => Err(SimError::AlreadyInitialized),
            (None, _) => Err(SimError::NotInitialized),
            (Some(sim), cmd) => sim.dispatch(cmd),
        }
    }
}

impl Sim {
    fn init(_params: InitParams) -> Result<Self, SimError> {
        // TODO: validate square matrix, zero diagonal, positive off-diagonal, unique vessel ids,
        // in-range locations, dock_throughput length/positivity, manifest entries.
        Err(SimError::NotImplemented("init"))
    }

    /// Runs a command and returns its payload: the planned state for everything except `get_state_at`.
    fn dispatch(&mut self, cmd: Command) -> Result<Value, SimError> {
        match cmd {
            Command::Init(_) => unreachable!("init is handled by Engine"),
            Command::Tick(params) => self.tick(params),
            Command::SetDestination {
                vessel_id,
                destination,
            } => self.set_destination(&vessel_id, destination),
            Command::GetState {} => Ok(()),
            Command::GetStateAt { tick } => return self.get_state_at(tick),
            Command::Dock { vessel_id } => self.dock(&vessel_id),
            Command::Undock { vessel_id } => self.undock(&vessel_id),
            Command::Load {
                vessel_id,
                destination,
            } => self.load(&vessel_id, destination),
            Command::Unload {
                vessel_id,
                destination,
            } => self.unload(&vessel_id, destination),
            Command::Swap {
                vessel_id,
                load_destination,
                unload_destination,
            } => self.swap(&vessel_id, load_destination, unload_destination),
        }?;
        Ok(self.render_planned())
    }

    fn render_planned(&self) -> Value {
        protocol::render_world(&self.planned, self.config.cargo_enabled())
    }

    // TODO: validate manifest, advance planned → committed, apply arrivals, add manifest, push history.
    fn tick(&mut self, _params: TickParams) -> Result<(), SimError> {
        Err(SimError::NotImplemented("tick"))
    }

    // TODO: committed must be idle; arrives_at_tick = current tick + travel time.
    fn set_destination(
        &mut self,
        _vessel_id: &str,
        _destination: Location,
    ) -> Result<(), SimError> {
        Err(SimError::NotImplemented("set_destination"))
    }

    fn dock(&mut self, _vessel_id: &str) -> Result<(), SimError> {
        Err(SimError::NotImplemented("dock"))
    }

    fn undock(&mut self, _vessel_id: &str) -> Result<(), SimError> {
        Err(SimError::NotImplemented("undock"))
    }

    // TODO: load/unload/swap all go through one helper: apply to a scratch copy of planned,
    // count diff-based ops at the dock, then commit or reject.
    fn load(&mut self, _vessel_id: &str, _destination: Location) -> Result<(), SimError> {
        Err(SimError::NotImplemented("load"))
    }

    fn unload(&mut self, _vessel_id: &str, _destination: Location) -> Result<(), SimError> {
        Err(SimError::NotImplemented("unload"))
    }

    fn swap(
        &mut self,
        _vessel_id: &str,
        _load_destination: Location,
        _unload_destination: Location,
    ) -> Result<(), SimError> {
        Err(SimError::NotImplemented("swap"))
    }

    // TODO: bounds-check against current tick, render history[tick].
    fn get_state_at(&self, _tick: i64) -> Result<Value, SimError> {
        Err(SimError::NotImplemented("get_state_at"))
    }
}
