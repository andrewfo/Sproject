//! The actual sim rules live here (ticks, movement, cargo, history).
//!
//! A few things that should always hold:
//! - planned and committed are the same right after a tick
//! - idle/docked checks look at committed, cargo checks look at planned
//! - handlers check everything before changing anything, so an error leaves state alone

use std::collections::{BTreeSet, HashMap};

use indexmap::IndexMap;
use serde_json::Value;

use crate::error::SimError;
use crate::protocol::{self, Command, InitParams, ManifestEntry, TickParams};
use crate::state::{
    Config, Inventory, Location, Vessel, VesselId, VesselStatus, World, add_cargo, remove_cargo,
};

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
    /// Units each vessel has delivered since the last tick. A delivery leaves no trace in the
    /// vessel's cargo diff when it's paired with a load for the same destination, so `dock_ops`
    /// needs it to count those ops.
    deliveries: HashMap<VesselId, u64>,
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
        validate_travel_times(&params.travel_times)?;
        let num_locations = params.travel_times.len();
        match &params.dock_throughput {
            Some(throughput) => validate_dock_throughput(throughput, num_locations)?,
            None if !params.cargo_manifest.is_empty() => {
                return Err(SimError::InvalidInit(
                    "cargo_manifest requires dock_throughput".into(),
                ));
            }
            None => {}
        }

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

        validate_manifest(&params.cargo_manifest, num_locations).map_err(SimError::InvalidInit)?;
        let mut docks = vec![Inventory::new(); num_locations];
        apply_manifest(&mut docks, &params.cargo_manifest).map_err(SimError::InvalidInit)?;

        let world = World {
            tick: 0,
            vessels,
            docks,
        };
        Ok(Sim {
            config: Config {
                travel_times: params.travel_times,
                dock_throughput: params.dock_throughput,
            },
            committed: world.clone(),
            planned: world.clone(),
            history: vec![world],
            deliveries: HashMap::new(),
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
    /// The manifest lands after arrivals. A bad manifest rejects the tick and nothing advances.
    fn tick(&mut self, params: TickParams) -> Result<(), SimError> {
        if !self.config.cargo_enabled() && !params.cargo_manifest.is_empty() {
            return Err(SimError::InvalidManifest(
                "cargo_manifest requires dock_throughput".into(),
            ));
        }
        validate_manifest(&params.cargo_manifest, self.config.num_locations())
            .map_err(SimError::InvalidManifest)?;

        let mut next = self.planned.clone();
        next.tick += 1;
        for vessel in next.vessels.values_mut() {
            if let VesselStatus::Transit {
                to,
                arrives_at_tick,
                ..
            } = vessel.status
                && arrives_at_tick <= next.tick
            {
                vessel.status = VesselStatus::Idle { location: to };
            }
        }
        apply_manifest(&mut next.docks, &params.cargo_manifest)
            .map_err(SimError::InvalidManifest)?;

        self.committed = next.clone();
        self.planned = next.clone();
        self.history.push(next);
        self.deliveries.clear();
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
            let arrives_at_tick = self
                .committed
                .tick
                .checked_add(self.config.travel_times[from][destination])
                .ok_or(SimError::ArrivalOverflow {
                    from,
                    to: destination,
                })?;
            VesselStatus::Transit {
                from,
                to: destination,
                arrives_at_tick,
            }
        };
        self.planned.vessels[vessel_id].status = status;
        Ok(())
    }

    /// Overrides a pending move. On a vessel that's already docked it cancels a pending undock,
    /// and docking twice is a no-op.
    fn dock(&mut self, vessel_id: &str) -> Result<(), SimError> {
        let location = match self.committed_vessel(vessel_id)?.status {
            VesselStatus::Idle { location } | VesselStatus::Docked { location } => location,
            VesselStatus::Transit { .. } => return Err(SimError::NotIdle(vessel_id.to_owned())),
        };
        self.planned.vessels[vessel_id].status = VesselStatus::Docked { location };
        Ok(())
    }

    /// Only a vessel docked at the last tick can undock. Pending cargo ops stay, and undocking twice is a no-op.
    fn undock(&mut self, vessel_id: &str) -> Result<(), SimError> {
        let VesselStatus::Docked { location } = self.committed_vessel(vessel_id)?.status else {
            return Err(SimError::NotDocked(vessel_id.to_owned()));
        };
        self.planned.vessels[vessel_id].status = VesselStatus::Idle { location };
        Ok(())
    }

    /// Shared by load/unload/swap. Applies the load (dock → vessel) and then the unload (vessel →
    /// dock, or delivered if it's for this location) to a scratch copy of planned, counts the dock's
    /// ops against throughput, and only keeps the copy if everything passed. A vessel with a
    /// pending undock can't transfer, since it's planned to leave.
    #[allow(dead_code)] // load/unload/swap call it next
    fn transfer(
        &mut self,
        vessel_id: &str,
        load: Option<Location>,
        unload: Option<Location>,
    ) -> Result<(), SimError> {
        let vessel = self.committed_vessel(vessel_id)?;
        for dest in load.into_iter().chain(unload) {
            self.check_location(dest)?;
        }
        let VesselStatus::Docked { location } = vessel.status else {
            return Err(SimError::NotDocked(vessel_id.to_owned()));
        };
        if self.planned.vessels[vessel_id].status != (VesselStatus::Docked { location }) {
            return Err(SimError::NotDocked(vessel_id.to_owned()));
        }

        let mut scratch = self.planned.clone();
        let dock_name = || format!("dock at location {location}");
        let vessel_name = || format!("vessel {vessel_id}");
        if let Some(dest) = load {
            if !remove_cargo(&mut scratch.docks[location], dest, 1) {
                return Err(SimError::InsufficientCargo {
                    what: dock_name(),
                    destination: dest,
                });
            }
            if !add_cargo(&mut scratch.vessels[vessel_id].cargo, dest, 1) {
                return Err(SimError::CargoOverflow {
                    what: vessel_name(),
                    destination: dest,
                });
            }
        }
        if let Some(dest) = unload {
            if !remove_cargo(&mut scratch.vessels[vessel_id].cargo, dest, 1) {
                return Err(SimError::InsufficientCargo {
                    what: vessel_name(),
                    destination: dest,
                });
            }
            if dest != location && !add_cargo(&mut scratch.docks[location], dest, 1) {
                return Err(SimError::CargoOverflow {
                    what: dock_name(),
                    destination: dest,
                });
            }
        }

        let mut deliveries = self.deliveries.clone();
        if unload == Some(location) {
            *deliveries.entry(vessel_id.to_owned()).or_default() += 1;
        }

        let ops = dock_ops(location, &self.committed, &scratch, &deliveries);
        // base mode has no throughput, so no transfer can fit
        let cap = self
            .config
            .dock_throughput
            .as_ref()
            .map_or(0, |throughput| throughput[location]);
        if ops > u64::from(cap) {
            return Err(SimError::ThroughputExceeded { ops, cap, location });
        }
        self.planned = scratch;
        self.deliveries = deliveries;
        Ok(())
    }

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

/// One positive throughput per location.
fn validate_dock_throughput(throughput: &[u32], num_locations: usize) -> Result<(), SimError> {
    if throughput.len() != num_locations {
        return Err(SimError::InvalidInit(format!(
            "dock_throughput has {} entries, expected {num_locations}",
            throughput.len()
        )));
    }
    if let Some(i) = throughput.iter().position(|&cap| cap == 0) {
        return Err(SimError::InvalidInit(format!(
            "dock_throughput[{i}] must be positive"
        )));
    }
    Ok(())
}

/// Checks every entry up front so a bad one rejects the whole manifest. Shared by `init` and `tick`,
/// which wrap the reason in their own error.
fn validate_manifest(entries: &[ManifestEntry], num_locations: usize) -> Result<(), String> {
    for (i, entry) in entries.iter().enumerate() {
        for (field, loc) in [
            ("location", entry.location),
            ("destination", entry.destination),
        ] {
            if loc >= num_locations {
                return Err(format!(
                    "cargo_manifest[{i}] {field} {loc} is out of range (0 to {})",
                    num_locations - 1
                ));
            }
        }
        if entry.quantity == 0 {
            return Err(format!("cargo_manifest[{i}] quantity must be positive"));
        }
    }
    Ok(())
}

/// Counts the transfer ops pending at a dock, by diffing each vessel's planned cargo against its
/// committed cargo. A vessel's ops are `max(loads, unloads)` since a swap pairs one of each into a
/// single op, so loading and then unloading the same unit nets to zero. Delivered units are gone
/// from the diff, so they're added back as if still on board and counted as unloads too: loading a
/// unit for this location and then delivering it is real work, not a no-op. Only vessels docked
/// there at the last tick count, because they're the only ones that can transfer. Sums are u64 so
/// huge quantities can't overflow.
fn dock_ops(
    location: Location,
    committed: &World,
    planned: &World,
    deliveries: &HashMap<VesselId, u64>,
) -> u64 {
    committed
        .vessels
        .iter()
        .filter(|(_, v)| v.status == VesselStatus::Docked { location })
        .map(|(id, before)| {
            let after = &planned.vessels[id].cargo;
            let delivered = deliveries.get(id).copied().unwrap_or(0);
            let mut dests: BTreeSet<Location> =
                before.cargo.keys().chain(after.keys()).copied().collect();
            if delivered > 0 {
                dests.insert(location);
            }
            let (mut loads, mut unloads) = (0u64, 0u64);
            for dest in dests {
                let was = u64::from(before.cargo.get(&dest).copied().unwrap_or(0));
                let mut now = u64::from(after.get(&dest).copied().unwrap_or(0));
                if dest == location {
                    now += delivered;
                    unloads += delivered;
                }
                loads += now.saturating_sub(was);
                unloads += was.saturating_sub(now);
            }
            loads.max(unloads)
        })
        .sum()
}

/// Adds an already-validated manifest to the docks. Callers pass a scratch copy, so an overflow
/// partway through doesn't leak into real state.
fn apply_manifest(docks: &mut [Inventory], entries: &[ManifestEntry]) -> Result<(), String> {
    for (i, entry) in entries.iter().enumerate() {
        if !add_cargo(
            &mut docks[entry.location],
            entry.destination,
            entry.quantity,
        ) {
            return Err(format!(
                "cargo_manifest[{i}] overflows the quantity for destination {} at location {}",
                entry.destination, entry.location
            ));
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

    #[test]
    fn set_destination_rejects_arrival_overflow() {
        let mut e = Engine::new();
        e.handle_line(&init_line(
            "[[0,18446744073709551615],[1,0]]",
            r#"[{"id":"v1","location":0}]"#,
        ));
        e.handle_line(TICK);
        assert_error(
            &e.handle_line(&set_dest("v1", 1)),
            "travel from 0 to 1 overflows the tick counter",
        );
        assert_eq!(planned_status(&e, "v1"), VesselStatus::Idle { location: 0 });
    }

    #[test]
    fn overdue_transit_arrives_on_next_tick() {
        let mut e = initialized();
        let sim = e.sim.as_mut().unwrap();
        sim.planned.vessels["v1"].status = VesselStatus::Transit {
            from: 0,
            to: 1,
            arrives_at_tick: 0,
        };
        e.handle_line(TICK);
        assert_eq!(
            committed_status(&e, "v1"),
            VesselStatus::Idle { location: 1 }
        );
    }

    fn cargo_init_line(throughput: &str, manifest: &str) -> String {
        format!(
            r#"{{"command":"init","parameters":{{"travel_times":{TT},"dock_throughput":{throughput},"vessels":[{{"id":"v1","location":0}}],"cargo_manifest":{manifest}}}}}"#
        )
    }

    #[test]
    fn cargo_init_renders_initial_cargo() {
        let mut e = Engine::new();
        assert_eq!(
            e.handle_line(&cargo_init_line(
                "[2,4,1]",
                r#"[{"location":1,"destination":0,"quantity":3},{"location":0,"destination":2,"quantity":1},{"location":0,"destination":1,"quantity":2},{"location":0,"destination":2,"quantity":4}]"#,
            )),
            r#"{"status":"ok","payload":{"tick":0,"vessels":{"v1":{"state":"idle","location":0,"cargo":[]}},"cargo":[{"location":0,"destination":2,"quantity":5},{"location":0,"destination":1,"quantity":2},{"location":1,"destination":0,"quantity":3}]}}"#
        );
    }

    #[test]
    fn cargo_init_without_manifest_has_empty_cargo() {
        let mut e = Engine::new();
        let v: Value =
            serde_json::from_str(&e.handle_line(&cargo_init_line("[2,4,1]", "[]"))).unwrap();
        assert_eq!(v["payload"]["cargo"], serde_json::json!([]));
        assert_eq!(
            v["payload"]["vessels"]["v1"]["cargo"],
            serde_json::json!([])
        );
    }

    #[test]
    fn base_init_has_no_cargo_fields() {
        let mut e = Engine::new();
        let v: Value = serde_json::from_str(&e.handle_line(&std_init())).unwrap();
        assert!(v["payload"].get("cargo").is_none());
        assert!(v["payload"]["vessels"]["v1"].get("cargo").is_none());
    }

    #[test]
    fn init_rejects_wrong_throughput_length() {
        let mut e = Engine::new();
        assert_error(
            &e.handle_line(&cargo_init_line("[2,4]", "[]")),
            "invalid init: dock_throughput has 2 entries, expected 3",
        );
        assert!(e.sim.is_none());
    }

    #[test]
    fn init_rejects_zero_throughput() {
        let mut e = Engine::new();
        assert_error(
            &e.handle_line(&cargo_init_line("[2,0,1]", "[]")),
            "invalid init: dock_throughput[1] must be positive",
        );
    }

    #[test]
    fn init_rejects_manifest_without_throughput() {
        let mut e = Engine::new();
        let line = r#"{"command":"init","parameters":{"travel_times":[[0]],"vessels":[],"cargo_manifest":[{"location":0,"destination":0,"quantity":1}]}}"#;
        assert_error(
            &e.handle_line(line),
            "invalid init: cargo_manifest requires dock_throughput",
        );
    }

    #[test]
    fn bad_manifest_entry_rejects_whole_init() {
        let cases = [
            (
                r#"[{"location":0,"destination":1,"quantity":1},{"location":3,"destination":1,"quantity":1}]"#,
                "invalid init: cargo_manifest[1] location 3 is out of range (0 to 2)",
            ),
            (
                r#"[{"location":0,"destination":5,"quantity":1}]"#,
                "invalid init: cargo_manifest[0] destination 5 is out of range (0 to 2)",
            ),
            (
                r#"[{"location":0,"destination":1,"quantity":2},{"location":1,"destination":2,"quantity":0}]"#,
                "invalid init: cargo_manifest[1] quantity must be positive",
            ),
        ];
        for (manifest, message) in cases {
            let mut e = Engine::new();
            assert_error(
                &e.handle_line(&cargo_init_line("[2,4,1]", manifest)),
                message,
            );
            assert!(e.sim.is_none());
        }
    }

    #[test]
    fn cargo_init_seeds_docks_in_committed_and_history() {
        let mut e = Engine::new();
        e.handle_line(&cargo_init_line(
            "[2,4,1]",
            r#"[{"location":2,"destination":0,"quantity":1}]"#,
        ));
        let sim = e.sim.as_ref().unwrap();
        assert_eq!(sim.committed, sim.planned);
        assert_eq!(sim.history, vec![sim.committed.clone()]);
        assert_eq!(sim.committed.docks[2], Inventory::from([(0, 1)]));
    }

    fn tick_with(manifest: &str) -> String {
        format!(r#"{{"command":"tick","parameters":{{"cargo_manifest":{manifest}}}}}"#)
    }

    fn cargo_initialized() -> Engine {
        let mut e = Engine::new();
        e.handle_line(&cargo_init_line("[2,4,1]", "[]"));
        e
    }

    #[test]
    fn tick_manifest_shows_up_in_committed_state() {
        let mut e = cargo_initialized();
        let out: Value = serde_json::from_str(&e.handle_line(&tick_with(
            r#"[{"location":0,"destination":2,"quantity":1},{"location":0,"destination":1,"quantity":1}]"#,
        )))
        .unwrap();
        assert_eq!(
            out["payload"]["cargo"],
            serde_json::json!([
                { "location": 0, "destination": 2, "quantity": 1 },
                { "location": 0, "destination": 1, "quantity": 1 },
            ])
        );
        let sim = e.sim.as_ref().unwrap();
        assert_eq!(sim.committed, sim.planned);
        assert_eq!(sim.history[1], sim.committed);
        // tick 0 in history doesn't pick up the new cargo
        assert!(sim.history[0].docks[0].is_empty());
    }

    #[test]
    fn tick_manifest_merges_repeated_destinations_in_order() {
        let mut e = cargo_initialized();
        e.handle_line(&tick_with(
            r#"[{"location":1,"destination":2,"quantity":1},{"location":1,"destination":0,"quantity":2}]"#,
        ));
        e.handle_line(&tick_with(
            r#"[{"location":1,"destination":0,"quantity":3},{"location":1,"destination":2,"quantity":4}]"#,
        ));
        assert_eq!(
            e.sim.as_ref().unwrap().committed.docks[1],
            Inventory::from([(2, 5), (0, 5)])
        );
    }

    #[test]
    fn bad_tick_manifest_leaves_tick_and_state_alone() {
        let cases = [
            (
                r#"[{"location":0,"destination":1,"quantity":1},{"location":9,"destination":1,"quantity":1}]"#,
                "invalid manifest: cargo_manifest[1] location 9 is out of range (0 to 2)",
            ),
            (
                r#"[{"location":0,"destination":1,"quantity":0}]"#,
                "invalid manifest: cargo_manifest[0] quantity must be positive",
            ),
        ];
        for (manifest, message) in cases {
            let mut e = cargo_initialized();
            e.handle_line(&set_dest("v1", 1));
            let before_planned = e.sim.as_ref().unwrap().planned.clone();
            assert_error(&e.handle_line(&tick_with(manifest)), message);
            let sim = e.sim.as_ref().unwrap();
            assert_eq!(sim.committed.tick, 0);
            assert_eq!(sim.planned, before_planned);
            assert_eq!(sim.history.len(), 1);
        }
    }

    #[test]
    fn tick_manifest_overflow_is_rejected() {
        let mut e = cargo_initialized();
        e.handle_line(&tick_with(
            r#"[{"location":0,"destination":1,"quantity":4294967295}]"#,
        ));
        assert_error(
            &e.handle_line(&tick_with(
                r#"[{"location":2,"destination":0,"quantity":1},{"location":0,"destination":1,"quantity":1}]"#,
            )),
            "invalid manifest: cargo_manifest[1] overflows the quantity for destination 1 at location 0",
        );
        let sim = e.sim.as_ref().unwrap();
        assert_eq!(sim.committed.tick, 1);
        // the first entry didn't sneak in either
        assert!(sim.planned.docks[2].is_empty());
    }

    #[test]
    fn init_manifest_overflow_is_rejected() {
        let mut e = Engine::new();
        assert_error(
            &e.handle_line(&cargo_init_line(
                "[2,4,1]",
                r#"[{"location":0,"destination":1,"quantity":4294967295},{"location":0,"destination":1,"quantity":1}]"#,
            )),
            "invalid init: cargo_manifest[1] overflows the quantity for destination 1 at location 0",
        );
        assert!(e.sim.is_none());
    }

    #[test]
    fn base_mode_tick_rejects_manifest() {
        let mut e = initialized();
        assert_error(
            &e.handle_line(&tick_with(
                r#"[{"location":0,"destination":1,"quantity":1}]"#,
            )),
            "invalid manifest: cargo_manifest requires dock_throughput",
        );
        assert_eq!(e.sim.as_ref().unwrap().committed.tick, 0);
        // an empty manifest is fine in base mode
        let v: Value = serde_json::from_str(&e.handle_line(&tick_with("[]"))).unwrap();
        assert_eq!(v["payload"]["tick"], 1);
    }

    fn dock(vessel: &str) -> String {
        format!(r#"{{"command":"dock","parameters":{{"vessel_id":"{vessel}"}}}}"#)
    }

    fn undock(vessel: &str) -> String {
        format!(r#"{{"command":"undock","parameters":{{"vessel_id":"{vessel}"}}}}"#)
    }

    /// v1 docked at location 0 as of the last tick.
    fn docked_engine() -> Engine {
        let mut e = cargo_initialized();
        e.handle_line(&dock("v1"));
        e.handle_line(TICK);
        e
    }

    #[test]
    fn dock_from_idle_plans_docked() {
        let mut e = cargo_initialized();
        let out: Value = serde_json::from_str(&e.handle_line(&dock("v1"))).unwrap();
        assert_eq!(
            out["payload"]["vessels"]["v1"],
            serde_json::json!({ "state": "docked", "location": 0, "cargo": [] })
        );
        assert_eq!(
            committed_status(&e, "v1"),
            VesselStatus::Idle { location: 0 }
        );
        e.handle_line(TICK);
        assert_eq!(
            committed_status(&e, "v1"),
            VesselStatus::Docked { location: 0 }
        );
    }

    #[test]
    fn dock_overrides_pending_set_destination() {
        let mut e = cargo_initialized();
        e.handle_line(&set_dest("v1", 1));
        e.handle_line(&dock("v1"));
        assert_eq!(
            planned_status(&e, "v1"),
            VesselStatus::Docked { location: 0 }
        );
    }

    #[test]
    fn set_destination_overrides_pending_dock() {
        let mut e = cargo_initialized();
        e.handle_line(&dock("v1"));
        e.handle_line(&set_dest("v1", 1));
        assert_eq!(
            planned_status(&e, "v1"),
            VesselStatus::Transit {
                from: 0,
                to: 1,
                arrives_at_tick: 4
            }
        );
    }

    #[test]
    fn set_destination_to_current_location_cancels_pending_dock() {
        let mut e = cargo_initialized();
        e.handle_line(&dock("v1"));
        e.handle_line(&set_dest("v1", 0));
        assert_eq!(planned_status(&e, "v1"), VesselStatus::Idle { location: 0 });
    }

    #[test]
    fn dock_twice_is_a_no_op() {
        let mut e = cargo_initialized();
        let first = e.handle_line(&dock("v1"));
        assert_eq!(e.handle_line(&dock("v1")), first);

        let mut e = docked_engine();
        let before = e.handle_line(GET_STATE);
        assert_eq!(e.handle_line(&dock("v1")), before);
    }

    #[test]
    fn dock_rejects_committed_transit() {
        let mut e = cargo_initialized();
        e.handle_line(&set_dest("v1", 1));
        e.handle_line(TICK);
        assert_error(&e.handle_line(&dock("v1")), "vessel v1 is not idle");
        assert!(matches!(
            planned_status(&e, "v1"),
            VesselStatus::Transit { .. }
        ));
    }

    #[test]
    fn dock_rejects_unknown_vessel() {
        let mut e = cargo_initialized();
        assert_error(&e.handle_line(&dock("nope")), "unknown vessel nope");
    }

    #[test]
    fn undock_plans_idle() {
        let mut e = docked_engine();
        e.handle_line(&undock("v1"));
        assert_eq!(planned_status(&e, "v1"), VesselStatus::Idle { location: 0 });
        assert_eq!(
            committed_status(&e, "v1"),
            VesselStatus::Docked { location: 0 }
        );
        e.handle_line(TICK);
        assert_eq!(
            committed_status(&e, "v1"),
            VesselStatus::Idle { location: 0 }
        );
    }

    #[test]
    fn undock_twice_is_a_no_op() {
        let mut e = docked_engine();
        let first = e.handle_line(&undock("v1"));
        assert_eq!(e.handle_line(&undock("v1")), first);
    }

    #[test]
    fn dock_after_undock_cancels_the_undock() {
        let mut e = docked_engine();
        e.handle_line(&undock("v1"));
        e.handle_line(&dock("v1"));
        assert_eq!(
            planned_status(&e, "v1"),
            VesselStatus::Docked { location: 0 }
        );
    }

    #[test]
    fn undock_rejects_pending_dock() {
        // committed is still idle, so there's nothing to undock from yet
        let mut e = cargo_initialized();
        e.handle_line(&dock("v1"));
        assert_error(&e.handle_line(&undock("v1")), "vessel v1 is not docked");
        assert_eq!(
            planned_status(&e, "v1"),
            VesselStatus::Docked { location: 0 }
        );
    }

    #[test]
    fn undock_rejects_idle_and_transit() {
        let mut e = cargo_initialized();
        assert_error(&e.handle_line(&undock("v1")), "vessel v1 is not docked");
        e.handle_line(&set_dest("v1", 1));
        e.handle_line(TICK);
        assert_error(&e.handle_line(&undock("v1")), "vessel v1 is not docked");
    }

    #[test]
    fn undock_rejects_unknown_vessel() {
        let mut e = cargo_initialized();
        assert_error(&e.handle_line(&undock("nope")), "unknown vessel nope");
    }

    #[test]
    fn set_destination_rejects_committed_docked() {
        let mut e = docked_engine();
        assert_error(&e.handle_line(&set_dest("v1", 1)), "vessel v1 is not idle");
        // even with an undock pending, it has to wait for the tick
        e.handle_line(&undock("v1"));
        assert_error(&e.handle_line(&set_dest("v1", 1)), "vessel v1 is not idle");
        assert_eq!(planned_status(&e, "v1"), VesselStatus::Idle { location: 0 });
    }

    fn vessel(status: VesselStatus, cargo: &[(Location, u32)]) -> Vessel {
        Vessel {
            status,
            cargo: cargo.iter().copied().collect(),
        }
    }

    fn world(vessels: Vec<(&str, Vessel)>) -> World {
        World {
            tick: 0,
            vessels: vessels
                .into_iter()
                .map(|(id, v)| (id.to_owned(), v))
                .collect(),
            docks: vec![Inventory::new(); 3],
        }
    }

    const DOCKED_0: VesselStatus = VesselStatus::Docked { location: 0 };

    #[test]
    fn dock_ops_counts_a_load() {
        let before = world(vec![("v1", vessel(DOCKED_0, &[]))]);
        let after = world(vec![("v1", vessel(DOCKED_0, &[(2, 1)]))]);
        assert_eq!(dock_ops(0, &before, &after, &HashMap::new()), 1);
    }

    #[test]
    fn dock_ops_counts_quantities_not_entries() {
        let before = world(vec![("v1", vessel(DOCKED_0, &[(1, 1)]))]);
        let after = world(vec![("v1", vessel(DOCKED_0, &[(1, 3), (2, 2)]))]);
        assert_eq!(dock_ops(0, &before, &after, &HashMap::new()), 4);
    }

    #[test]
    fn dock_ops_counts_a_swap_as_one() {
        let before = world(vec![("v1", vessel(DOCKED_0, &[(1, 1)]))]);
        let after = world(vec![("v1", vessel(DOCKED_0, &[(2, 1)]))]);
        assert_eq!(dock_ops(0, &before, &after, &HashMap::new()), 1);
    }

    #[test]
    fn dock_ops_uneven_swap_takes_the_bigger_side() {
        let before = world(vec![("v1", vessel(DOCKED_0, &[(1, 1)]))]);
        let after = world(vec![("v1", vessel(DOCKED_0, &[(2, 3)]))]);
        assert_eq!(dock_ops(0, &before, &after, &HashMap::new()), 3);
    }

    #[test]
    fn dock_ops_load_then_unload_same_cargo_is_zero() {
        let before = world(vec![("v1", vessel(DOCKED_0, &[(1, 2)]))]);
        assert_eq!(dock_ops(0, &before, &before.clone(), &HashMap::new()), 0);
    }

    #[test]
    fn dock_ops_adds_up_vessels_at_the_same_dock() {
        let before = world(vec![
            ("v1", vessel(DOCKED_0, &[])),
            ("v2", vessel(DOCKED_0, &[(1, 1)])),
        ]);
        let after = world(vec![
            ("v1", vessel(DOCKED_0, &[(2, 1)])),
            ("v2", vessel(DOCKED_0, &[])),
        ]);
        assert_eq!(dock_ops(0, &before, &after, &HashMap::new()), 2);
    }

    #[test]
    fn dock_ops_ignores_other_docks_and_undocked_vessels() {
        let before = world(vec![
            ("v1", vessel(VesselStatus::Docked { location: 1 }, &[])),
            ("v2", vessel(VesselStatus::Idle { location: 0 }, &[])),
        ]);
        let after = world(vec![
            (
                "v1",
                vessel(VesselStatus::Docked { location: 1 }, &[(2, 1)]),
            ),
            ("v2", vessel(VesselStatus::Docked { location: 0 }, &[])),
        ]);
        assert_eq!(dock_ops(0, &before, &after, &HashMap::new()), 0);
        assert_eq!(dock_ops(1, &before, &after, &HashMap::new()), 1);
    }

    #[test]
    fn dock_ops_counts_a_pending_undock() {
        // loads made before an undock still happen this tick
        let before = world(vec![("v1", vessel(DOCKED_0, &[]))]);
        let after = world(vec![(
            "v1",
            vessel(VesselStatus::Idle { location: 0 }, &[(2, 1)]),
        )]);
        assert_eq!(dock_ops(0, &before, &after, &HashMap::new()), 1);
    }

    #[test]
    fn dock_ops_does_not_overflow() {
        let before = world(vec![("v1", vessel(DOCKED_0, &[]))]);
        let after = world(vec![(
            "v1",
            vessel(DOCKED_0, &[(1, u32::MAX), (2, u32::MAX)]),
        )]);
        assert_eq!(
            dock_ops(0, &before, &after, &HashMap::new()),
            2 * u64::from(u32::MAX)
        );
    }

    /// v1 docked at location 0 (throughput 2), with 2 units for 2 and 1 for 1 on the dock.
    fn stocked_engine() -> Engine {
        let mut e = Engine::new();
        e.handle_line(&cargo_init_line(
            "[2,4,1]",
            r#"[{"location":0,"destination":2,"quantity":2},{"location":0,"destination":1,"quantity":1}]"#,
        ));
        e.handle_line(&dock("v1"));
        e.handle_line(TICK);
        e
    }

    fn sim(e: &mut Engine) -> &mut Sim {
        e.sim.as_mut().unwrap()
    }

    fn total_cargo(world: &World) -> u32 {
        let on_docks: u32 = world.docks.iter().flat_map(|d| d.values()).sum();
        let on_vessels: u32 = world.vessels.values().flat_map(|v| v.cargo.values()).sum();
        on_docks + on_vessels
    }

    #[test]
    fn transfer_load_moves_dock_cargo_onto_the_vessel() {
        let mut e = stocked_engine();
        let s = sim(&mut e);
        s.transfer("v1", Some(2), None).unwrap();
        assert_eq!(s.planned.vessels["v1"].cargo, Inventory::from([(2, 1)]));
        assert_eq!(s.planned.docks[0], Inventory::from([(2, 1), (1, 1)]));
        assert_eq!(total_cargo(&s.planned), total_cargo(&s.committed));
    }

    #[test]
    fn transfer_unload_moves_vessel_cargo_onto_the_dock() {
        let mut e = stocked_engine();
        let s = sim(&mut e);
        s.transfer("v1", Some(1), None).unwrap();
        s.transfer("v1", None, Some(1)).unwrap();
        assert_eq!(s.planned.vessels["v1"].cargo, Inventory::new());
        // the unit went back on the end, after dest 2
        assert_eq!(s.planned.docks[0], Inventory::from([(2, 2), (1, 1)]));
        assert_eq!(total_cargo(&s.planned), total_cargo(&s.committed));
    }

    #[test]
    fn transfer_unload_at_its_destination_delivers() {
        let mut e = stocked_engine();
        let s = sim(&mut e);
        s.committed.vessels["v1"].cargo = Inventory::from([(0, 1)]);
        s.planned = s.committed.clone();
        s.transfer("v1", None, Some(0)).unwrap();
        assert_eq!(s.planned.vessels["v1"].cargo, Inventory::new());
        assert!(!s.planned.docks[0].contains_key(&0));
        assert_eq!(total_cargo(&s.planned), total_cargo(&s.committed) - 1);
    }

    #[test]
    fn transfer_swap_conserves_cargo_and_counts_one_op() {
        let mut e = stocked_engine();
        let s = sim(&mut e);
        s.transfer("v1", Some(2), None).unwrap();
        s.transfer("v1", Some(1), Some(2)).unwrap();
        assert_eq!(s.planned.vessels["v1"].cargo, Inventory::from([(1, 1)]));
        assert_eq!(dock_ops(0, &s.committed, &s.planned, &s.deliveries), 1);
        assert_eq!(total_cargo(&s.planned), total_cargo(&s.committed));
    }

    #[test]
    fn transfer_rejects_a_missing_dock_unit() {
        let mut e = stocked_engine();
        let s = sim(&mut e);
        let before = s.planned.clone();
        assert_eq!(
            s.transfer("v1", Some(0), None).unwrap_err().to_string(),
            "dock at location 0 has no cargo for destination 0"
        );
        assert_eq!(s.planned, before);
    }

    #[test]
    fn transfer_swap_with_a_missing_unload_unit_leaves_planned_alone() {
        let mut e = stocked_engine();
        let s = sim(&mut e);
        let before = s.planned.clone();
        // the load half would work, so this checks it got thrown away
        assert_eq!(
            s.transfer("v1", Some(2), Some(1)).unwrap_err().to_string(),
            "vessel v1 has no cargo for destination 1"
        );
        assert_eq!(s.planned, before);
    }

    #[test]
    fn transfer_rejects_going_over_throughput() {
        let mut e = stocked_engine();
        let s = sim(&mut e);
        s.transfer("v1", Some(2), None).unwrap();
        s.transfer("v1", Some(2), None).unwrap();
        let before = s.planned.clone();
        assert_eq!(
            s.transfer("v1", Some(1), None).unwrap_err().to_string(),
            "transfer of 3 operations exceeds dock throughput of 2 at location 0"
        );
        assert_eq!(s.planned, before);
    }

    #[test]
    fn transfer_rejects_vessels_not_docked_at_the_last_tick() {
        let mut e = stocked_engine();
        e.handle_line(&undock("v1"));
        let s = sim(&mut e);
        let before = s.planned.clone();
        assert_eq!(
            s.transfer("v1", Some(2), None).unwrap_err().to_string(),
            "vessel v1 is not docked"
        );
        assert_eq!(s.planned, before);
        // docked only in the plan isn't enough either
        let mut e = cargo_initialized();
        e.handle_line(&dock("v1"));
        assert_eq!(
            sim(&mut e)
                .transfer("v1", Some(2), None)
                .unwrap_err()
                .to_string(),
            "vessel v1 is not docked"
        );
    }

    #[test]
    fn transfer_checks_vessel_then_location_then_docked() {
        let mut e = stocked_engine();
        let s = sim(&mut e);
        assert_eq!(
            s.transfer("nope", Some(9), None).unwrap_err().to_string(),
            "unknown vessel nope"
        );
        assert_eq!(
            s.transfer("v1", None, Some(9)).unwrap_err().to_string(),
            "location 9 is out of range"
        );
    }

    #[test]
    fn transfer_rejects_a_vessel_cargo_overflow() {
        let mut e = stocked_engine();
        let s = sim(&mut e);
        s.committed.vessels["v1"].cargo = Inventory::from([(2, u32::MAX)]);
        s.planned = s.committed.clone();
        let before = s.planned.clone();
        assert_eq!(
            s.transfer("v1", Some(2), None).unwrap_err().to_string(),
            "vessel v1 would overflow its cargo for destination 2"
        );
        assert_eq!(s.planned, before);
    }

    /// v1 docked at location 0 (throughput 1), with 2 units already bound for location 0 on the dock.
    fn own_dest_engine() -> Engine {
        let mut e = Engine::new();
        e.handle_line(&cargo_init_line(
            "[1,4,1]",
            r#"[{"location":0,"destination":0,"quantity":2}]"#,
        ));
        e.handle_line(&dock("v1"));
        e.handle_line(TICK);
        e
    }

    #[test]
    fn delivery_counts_as_one_op() {
        let mut e = own_dest_engine();
        let s = sim(&mut e);
        s.committed.vessels["v1"].cargo = Inventory::from([(0, 1)]);
        s.planned = s.committed.clone();
        s.transfer("v1", None, Some(0)).unwrap();
        assert_eq!(dock_ops(0, &s.committed, &s.planned, &s.deliveries), 1);
    }

    #[test]
    fn load_then_deliver_the_same_unit_is_not_free() {
        let mut e = own_dest_engine();
        let s = sim(&mut e);
        s.transfer("v1", Some(0), None).unwrap();
        s.transfer("v1", None, Some(0)).unwrap();
        // the vessel's cargo is back where it was, but a unit left the sim
        assert_eq!(s.planned.vessels["v1"].cargo, Inventory::new());
        assert_eq!(s.planned.docks[0], Inventory::from([(0, 1)]));
        assert_eq!(dock_ops(0, &s.committed, &s.planned, &s.deliveries), 1);
        assert_eq!(
            s.transfer("v1", Some(0), None).unwrap_err().to_string(),
            "transfer of 2 operations exceeds dock throughput of 1 at location 0"
        );
    }

    #[test]
    fn swap_that_delivers_and_reloads_counts_one_op() {
        let mut e = own_dest_engine();
        let s = sim(&mut e);
        s.committed.vessels["v1"].cargo = Inventory::from([(0, 1)]);
        s.planned = s.committed.clone();
        s.transfer("v1", Some(0), Some(0)).unwrap();
        assert_eq!(s.planned.vessels["v1"].cargo, Inventory::from([(0, 1)]));
        assert_eq!(s.planned.docks[0], Inventory::from([(0, 1)]));
        assert_eq!(dock_ops(0, &s.committed, &s.planned, &s.deliveries), 1);
    }

    #[test]
    fn rejected_delivery_is_not_remembered() {
        let mut e = own_dest_engine();
        let s = sim(&mut e);
        s.committed.vessels["v1"].cargo = Inventory::from([(0, 2)]);
        s.planned = s.committed.clone();
        s.transfer("v1", None, Some(0)).unwrap();
        assert!(s.transfer("v1", None, Some(0)).is_err());
        assert_eq!(s.deliveries, HashMap::from([("v1".to_owned(), 1)]));
    }

    #[test]
    fn tick_forgets_deliveries() {
        let mut e = own_dest_engine();
        let s = sim(&mut e);
        s.committed.vessels["v1"].cargo = Inventory::from([(0, 1)]);
        s.planned = s.committed.clone();
        s.transfer("v1", None, Some(0)).unwrap();
        e.handle_line(TICK);
        assert!(sim(&mut e).deliveries.is_empty());
    }

    fn assert_invalid_command(response: &str, unknown_field: &str) {
        let v: Value = serde_json::from_str(response).unwrap();
        assert_eq!(v["status"], "error", "expected error, got {response}");
        let message = v["payload"]["message"].as_str().unwrap();
        assert!(
            message.starts_with(&format!("invalid command: unknown field `{unknown_field}`")),
            "{message}"
        );
    }

    #[test]
    fn misspelled_init_field_is_rejected() {
        let mut e = Engine::new();
        assert_invalid_command(
            &e.handle_line(
                r#"{"command":"init","parameters":{"travel_times":[[0]],"vessels":[],"dock_througput":[1]}}"#,
            ),
            "dock_througput",
        );
        assert!(e.sim.is_none());
    }

    #[test]
    fn misspelled_tick_field_does_not_advance() {
        let mut e = initialized();
        assert_invalid_command(
            &e.handle_line(r#"{"command":"tick","parameters":{"cargo_manfest":[]}}"#),
            "cargo_manfest",
        );
        assert_eq!(e.sim.as_ref().unwrap().committed.tick, 0);
    }

    #[test]
    fn extra_command_parameters_are_rejected() {
        let mut e = initialized();
        assert_invalid_command(
            &e.handle_line(
                r#"{"command":"set_destination","parameters":{"vessel_id":"v1","destination":1,"oops":2}}"#,
            ),
            "oops",
        );
        assert_eq!(planned_status(&e, "v1"), VesselStatus::Idle { location: 0 });
        assert_invalid_command(
            &e.handle_line(r#"{"command":"get_state","parameters":{"extra":1}}"#),
            "extra",
        );
    }
}
