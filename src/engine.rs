//! The actual sim rules live here (ticks, movement, cargo, history).
//!
//! A few things that should always hold:
//! - planned and committed are the same right after a tick
//! - idle/docked checks look at committed, cargo checks look at planned
//! - handlers check everything before changing anything, so an error leaves state alone

use indexmap::IndexMap;
use serde_json::Value;

use crate::error::SimError;
use crate::protocol::{self, Command, InitParams, TickParams};
use crate::state::{Config, Inventory, Location, Vessel, VesselStatus, World};

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
        protocol::render_response(protocol::parse_command(line).and_then(|cmd| self.handle(cmd)))
    }

    /// Runs a parsed command and returns the WorldState payload.
    fn handle(&mut self, cmd: Command) -> Result<Value, SimError> {
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
    fn init(params: InitParams) -> Result<Self, SimError> {
        // TODO: cargo mode — validate dock_throughput and cargo_manifest, seed docks from the manifest.
        if params.dock_throughput.is_some() || !params.cargo_manifest.is_empty() {
            return Err(SimError::NotImplemented("cargo"));
        }

        validate_travel_times(&params.travel_times)?;
        let num_locations = params.travel_times.len();

        let mut vessels = IndexMap::new();
        for spec in params.vessels {
            if spec.location >= num_locations {
                return Err(SimError::InvalidInit(format!(
                    "vessel {} location {} is out of range (0 to {})",
                    spec.id,
                    spec.location,
                    num_locations - 1
                )));
            }
            if vessels.contains_key(&spec.id) {
                return Err(SimError::InvalidInit(format!(
                    "duplicate vessel id {}",
                    spec.id
                )));
            }
            let vessel = Vessel {
                status: VesselStatus::Idle {
                    location: spec.location,
                },
                cargo: Inventory::new(),
            };
            vessels.insert(spec.id, vessel);
        }

        let world = World {
            tick: 0,
            vessels,
            docks: vec![Inventory::new(); num_locations],
        };
        Ok(Sim {
            config: Config {
                travel_times: params.travel_times,
                dock_throughput: None,
            },
            committed: world.clone(),
            planned: world.clone(),
            history: vec![world],
        })
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

    fn committed_vessel(&self, id: &str) -> Result<&Vessel, SimError> {
        self.committed
            .vessels
            .get(id)
            .ok_or_else(|| SimError::UnknownVessel(id.to_owned()))
    }

    fn check_location(&self, loc: Location) -> Result<(), SimError> {
        if loc < self.config.num_locations() {
            Ok(())
        } else {
            Err(SimError::LocationOutOfRange(loc))
        }
    }

    /// A vessel whose arrival tick is the new tick shows up as idle at its destination right away.
    // TODO: cargo mode — validate and add the tick's manifest.
    fn tick(&mut self, params: TickParams) -> Result<(), SimError> {
        if !params.cargo_manifest.is_empty() {
            return Err(SimError::NotImplemented("cargo"));
        }

        let mut next = self.planned.clone();
        next.tick += 1;
        for vessel in next.vessels.values_mut() {
            if let VesselStatus::Transit {
                to,
                arrives_at_tick,
                ..
            } = vessel.status
                && arrives_at_tick == next.tick
            {
                vessel.status = VesselStatus::Idle { location: to };
            }
        }

        self.committed = next.clone();
        self.planned = next.clone();
        self.history.push(next);
        Ok(())
    }

    /// Setting the destination to the vessel's current location cancels a pending move.
    fn set_destination(&mut self, vessel_id: &str, destination: Location) -> Result<(), SimError> {
        let vessel = self.committed_vessel(vessel_id)?;
        self.check_location(destination)?;
        let VesselStatus::Idle { location: from } = vessel.status else {
            return Err(SimError::NotIdle(vessel_id.to_owned()));
        };

        let status = if destination == from {
            VesselStatus::Idle { location: from }
        } else {
            VesselStatus::Transit {
                from,
                to: destination,
                arrives_at_tick: self.committed.tick + self.config.travel_times[from][destination],
            }
        };
        self.planned.vessels[vessel_id].status = status;
        Ok(())
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

/// N×N, zero diagonal, positive everywhere else.
fn validate_travel_times(travel_times: &[Vec<u64>]) -> Result<(), SimError> {
    let n = travel_times.len();
    if n == 0 {
        return Err(SimError::InvalidInit(
            "travel_times must have at least one location".into(),
        ));
    }
    for (i, row) in travel_times.iter().enumerate() {
        if row.len() != n {
            return Err(SimError::InvalidInit(format!(
                "travel_times must be square: row {i} has {} entries, expected {n}",
                row.len()
            )));
        }
        for (j, &t) in row.iter().enumerate() {
            if i == j && t != 0 {
                return Err(SimError::InvalidInit(format!(
                    "travel_times[{i}][{j}] must be 0"
                )));
            }
            if i != j && t == 0 {
                return Err(SimError::InvalidInit(format!(
                    "travel_times[{i}][{j}] must be positive"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TT: &str = "[[0,4,7],[4,0,3],[7,3,0]]";
    const GET_STATE: &str = r#"{"command":"get_state","parameters":{}}"#;
    const TICK: &str = r#"{"command":"tick","parameters":{}}"#;

    fn init_line(travel_times: &str, vessels: &str) -> String {
        format!(
            r#"{{"command":"init","parameters":{{"travel_times":{travel_times},"vessels":{vessels}}}}}"#
        )
    }

    fn std_init() -> String {
        init_line(TT, r#"[{"id":"v1","location":0},{"id":"v2","location":2}]"#)
    }

    fn initialized() -> Engine {
        let mut e = Engine::new();
        e.handle_line(&std_init());
        e
    }

    fn assert_error(response: &str, message: &str) {
        let v: Value = serde_json::from_str(response).unwrap();
        assert_eq!(v["status"], "error", "expected error, got {response}");
        assert_eq!(v["payload"]["message"], message);
    }

    #[test]
    fn init_renders_tick_zero() {
        let mut e = Engine::new();
        assert_eq!(
            e.handle_line(&std_init()),
            r#"{"status":"ok","payload":{"tick":0,"vessels":{"v1":{"state":"idle","location":0},"v2":{"state":"idle","location":2}}}}"#
        );
    }

    #[test]
    fn init_keeps_vessel_order() {
        let mut e = Engine::new();
        let out = e.handle_line(&init_line(
            TT,
            r#"[{"id":"zz","location":1},{"id":"aa","location":0}]"#,
        ));
        assert!(out.find("\"zz\"").unwrap() < out.find("\"aa\"").unwrap());
    }

    #[test]
    fn init_with_no_vessels_is_ok() {
        let mut e = Engine::new();
        assert_eq!(
            e.handle_line(&init_line("[[0]]", "[]")),
            r#"{"status":"ok","payload":{"tick":0,"vessels":{}}}"#
        );
    }

    #[test]
    fn init_rejects_empty_matrix() {
        let mut e = Engine::new();
        assert_error(
            &e.handle_line(&init_line("[]", "[]")),
            "invalid init: travel_times must have at least one location",
        );
    }

    #[test]
    fn init_rejects_non_square_matrix() {
        let mut e = Engine::new();
        assert_error(
            &e.handle_line(&init_line("[[0,1],[1,0,2]]", "[]")),
            "invalid init: travel_times must be square: row 1 has 3 entries, expected 2",
        );
    }

    #[test]
    fn init_rejects_nonzero_diagonal() {
        let mut e = Engine::new();
        assert_error(
            &e.handle_line(&init_line("[[0,1],[1,5]]", "[]")),
            "invalid init: travel_times[1][1] must be 0",
        );
    }

    #[test]
    fn init_rejects_zero_off_diagonal() {
        let mut e = Engine::new();
        assert_error(
            &e.handle_line(&init_line("[[0,0],[1,0]]", "[]")),
            "invalid init: travel_times[0][1] must be positive",
        );
    }

    #[test]
    fn init_rejects_negative_travel_time() {
        let mut e = Engine::new();
        let out = e.handle_line(&init_line("[[0,-1],[1,0]]", "[]"));
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["status"], "error");
        assert!(
            v["payload"]["message"]
                .as_str()
                .unwrap()
                .starts_with("invalid command:")
        );
    }

    #[test]
    fn init_rejects_duplicate_vessel_ids() {
        let mut e = Engine::new();
        assert_error(
            &e.handle_line(&init_line(
                TT,
                r#"[{"id":"v1","location":0},{"id":"v1","location":1}]"#,
            )),
            "invalid init: duplicate vessel id v1",
        );
    }

    #[test]
    fn init_rejects_out_of_range_location() {
        let mut e = Engine::new();
        assert_error(
            &e.handle_line(&init_line(TT, r#"[{"id":"v1","location":3}]"#)),
            "invalid init: vessel v1 location 3 is out of range (0 to 2)",
        );
    }

    #[test]
    fn second_init_is_rejected() {
        let mut e = initialized();
        assert_error(
            &e.handle_line(&std_init()),
            "simulation already initialized",
        );
    }

    #[test]
    fn commands_before_init_are_rejected() {
        let mut e = Engine::new();
        assert_error(
            &e.handle_line(GET_STATE),
            "simulation not initialized; send init first",
        );
    }

    #[test]
    fn failed_init_leaves_engine_uninitialized() {
        let mut e = Engine::new();
        e.handle_line(&init_line("[[0,1],[1,5]]", "[]"));
        assert_error(
            &e.handle_line(GET_STATE),
            "simulation not initialized; send init first",
        );
        // and a corrected init still goes through
        let v: Value = serde_json::from_str(&e.handle_line(&std_init())).unwrap();
        assert_eq!(v["status"], "ok");
    }

    #[test]
    fn init_seeds_committed_planned_and_history() {
        let e = initialized();
        let sim = e.sim.as_ref().unwrap();
        assert_eq!(sim.committed, sim.planned);
        assert_eq!(sim.history, vec![sim.committed.clone()]);
        assert_eq!(sim.committed.docks.len(), 3);
    }

    #[test]
    fn get_state_after_init_matches_init_payload() {
        let mut e = Engine::new();
        let init = e.handle_line(&std_init());
        assert_eq!(e.handle_line(GET_STATE), init);
    }

    fn set_dest(vessel: &str, destination: Location) -> String {
        format!(
            r#"{{"command":"set_destination","parameters":{{"vessel_id":"{vessel}","destination":{destination}}}}}"#
        )
    }

    fn planned_status(e: &Engine, vessel: &str) -> VesselStatus {
        e.sim.as_ref().unwrap().planned.vessels[vessel]
            .status
            .clone()
    }

    #[test]
    fn set_destination_plans_transit() {
        let mut e = initialized();
        let out: Value = serde_json::from_str(&e.handle_line(&set_dest("v1", 1))).unwrap();
        assert_eq!(
            out["payload"]["vessels"]["v1"],
            serde_json::json!({ "state": "transit", "from": 0, "to": 1, "arrives_at_tick": 4 })
        );
        // committed is untouched until the next tick
        let sim = e.sim.as_ref().unwrap();
        assert_eq!(
            sim.committed.vessels["v1"].status,
            VesselStatus::Idle { location: 0 }
        );
    }

    #[test]
    fn set_destination_second_call_wins() {
        let mut e = initialized();
        e.handle_line(&set_dest("v1", 1));
        e.handle_line(&set_dest("v1", 2));
        assert_eq!(
            planned_status(&e, "v1"),
            VesselStatus::Transit {
                from: 0,
                to: 2,
                arrives_at_tick: 7
            }
        );
    }

    #[test]
    fn set_destination_to_current_location_cancels() {
        let mut e = initialized();
        e.handle_line(&set_dest("v1", 1));
        e.handle_line(&set_dest("v1", 0));
        assert_eq!(planned_status(&e, "v1"), VesselStatus::Idle { location: 0 });
    }

    #[test]
    fn set_destination_arrival_uses_current_tick() {
        let mut e = initialized();
        for _ in 0..4 {
            e.handle_line(TICK);
        }
        e.handle_line(&set_dest("v2", 1));
        assert_eq!(
            planned_status(&e, "v2"),
            VesselStatus::Transit {
                from: 2,
                to: 1,
                arrives_at_tick: 7
            }
        );
    }

    #[test]
    fn set_destination_rejects_committed_transit() {
        let mut e = initialized();
        let transit = VesselStatus::Transit {
            from: 0,
            to: 1,
            arrives_at_tick: 4,
        };
        let sim = e.sim.as_mut().unwrap();
        sim.committed.vessels["v1"].status = transit.clone();
        sim.planned.vessels["v1"].status = transit.clone();
        assert_error(&e.handle_line(&set_dest("v1", 2)), "vessel v1 is not idle");
        assert_eq!(planned_status(&e, "v1"), transit);
    }

    #[test]
    fn set_destination_rejects_unknown_vessel() {
        let mut e = initialized();
        assert_error(&e.handle_line(&set_dest("nope", 1)), "unknown vessel nope");
    }

    #[test]
    fn set_destination_rejects_out_of_range() {
        let mut e = initialized();
        assert_error(
            &e.handle_line(&set_dest("v1", 3)),
            "location 3 is out of range",
        );
        assert_eq!(planned_status(&e, "v1"), VesselStatus::Idle { location: 0 });
    }

    #[test]
    fn set_destination_checks_vessel_before_location() {
        let mut e = initialized();
        assert_error(&e.handle_line(&set_dest("nope", 9)), "unknown vessel nope");
    }

    fn committed_status(e: &Engine, vessel: &str) -> VesselStatus {
        e.sim.as_ref().unwrap().committed.vessels[vessel]
            .status
            .clone()
    }

    #[test]
    fn tick_advances_and_commits_the_plan() {
        let mut e = initialized();
        e.handle_line(&set_dest("v1", 1));
        let out: Value = serde_json::from_str(&e.handle_line(TICK)).unwrap();
        assert_eq!(out["payload"]["tick"], 1);
        let sim = e.sim.as_ref().unwrap();
        assert_eq!(sim.committed, sim.planned);
        assert_eq!(sim.history.len(), 2);
        assert_eq!(sim.history[1], sim.committed);
        assert_eq!(
            committed_status(&e, "v1"),
            VesselStatus::Transit {
                from: 0,
                to: 1,
                arrives_at_tick: 4
            }
        );
    }

    #[test]
    fn vessel_is_idle_on_its_arrival_tick() {
        let mut e = initialized();
        e.handle_line(&set_dest("v1", 1));
        for _ in 0..3 {
            e.handle_line(TICK);
        }
        assert!(matches!(
            committed_status(&e, "v1"),
            VesselStatus::Transit { .. }
        ));
        e.handle_line(TICK);
        assert_eq!(
            committed_status(&e, "v1"),
            VesselStatus::Idle { location: 1 }
        );
    }

    #[test]
    fn vessel_can_be_recommanded_on_arrival_tick() {
        let mut e = initialized();
        e.handle_line(&set_dest("v1", 1));
        for _ in 0..4 {
            e.handle_line(TICK);
        }
        e.handle_line(&set_dest("v1", 2));
        assert_eq!(
            planned_status(&e, "v1"),
            VesselStatus::Transit {
                from: 1,
                to: 2,
                arrives_at_tick: 7
            }
        );
    }

    #[test]
    fn history_keeps_earlier_ticks() {
        let mut e = initialized();
        e.handle_line(&set_dest("v1", 1));
        e.handle_line(TICK);
        e.handle_line(TICK);
        let sim = e.sim.as_ref().unwrap();
        assert_eq!(
            sim.history[0].vessels["v1"].status,
            VesselStatus::Idle { location: 0 }
        );
        assert_eq!(sim.history[0].tick, 0);
        assert_eq!(sim.history[2].tick, 2);
    }

    #[test]
    fn tick_with_no_vessels_works() {
        let mut e = Engine::new();
        e.handle_line(&init_line("[[0]]", "[]"));
        assert_eq!(
            e.handle_line(TICK),
            r#"{"status":"ok","payload":{"tick":1,"vessels":{}}}"#
        );
    }
}
