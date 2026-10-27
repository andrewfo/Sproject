//! Wire format: command parsing and response rendering.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::SimError;
use crate::state::{Location, VesselId, VesselStatus, World};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "command", content = "parameters", rename_all = "snake_case")]
pub enum Command {
    Init(InitParams),
    Tick(TickParams),
    SetDestination {
        vessel_id: VesselId,
        destination: Location,
    },
    GetState {},
    Dock {
        vessel_id: VesselId,
    },
    Undock {
        vessel_id: VesselId,
    },
    Load {
        vessel_id: VesselId,
        destination: Location,
    },
    Unload {
        vessel_id: VesselId,
        destination: Location,
    },
    Swap {
        vessel_id: VesselId,
        load_destination: Location,
        unload_destination: Location,
    },
    GetStateAt {
        // Signed so a negative tick gets a clean error instead of a parse failure.
        tick: i64,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct InitParams {
    pub travel_times: Vec<Vec<u64>>,
    pub vessels: Vec<VesselSpec>,
    #[serde(default)]
    pub dock_throughput: Option<Vec<u32>>,
    #[serde(default)]
    pub cargo_manifest: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VesselSpec {
    pub id: VesselId,
    pub location: Location,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct TickParams {
    #[serde(default)]
    pub cargo_manifest: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestEntry {
    pub location: Location,
    pub destination: Location,
    pub quantity: u32,
}

pub fn parse_command(line: &str) -> Result<Command, SimError> {
    serde_json::from_str(line).map_err(|e| SimError::InvalidCommand(e.to_string()))
}

pub fn ok_response(payload: Value) -> Value {
    json!({ "status": "ok", "payload": payload })
}

pub fn error_response(err: &SimError) -> Value {
    json!({ "status": "error", "payload": { "message": err.to_string() } })
}

/// Renders a world to the spec's `WorldState` JSON. `cargo` fields appear only when cargo is enabled.
pub fn render_world(world: &World, cargo_enabled: bool) -> Value {
    let mut vessels = serde_json::Map::new();
    for (id, vessel) in &world.vessels {
        let mut v = match vessel.status {
            VesselStatus::Idle { location } => json!({ "state": "idle", "location": location }),
            VesselStatus::Docked { location } => json!({ "state": "docked", "location": location }),
            VesselStatus::Transit {
                from,
                to,
                arrives_at_tick,
            } => json!({
                "state": "transit",
                "from": from,
                "to": to,
                "arrives_at_tick": arrives_at_tick,
            }),
        };
        if cargo_enabled {
            let cargo: Vec<Value> = vessel
                .cargo
                .iter()
                .map(|(dest, qty)| json!({ "destination": dest, "quantity": qty }))
                .collect();
            v["cargo"] = Value::Array(cargo);
        }
        vessels.insert(id.clone(), v);
    }

    let mut out = json!({ "tick": world.tick, "vessels": vessels });
    if cargo_enabled {
        let docks: Vec<Value> = world
            .docks
            .iter()
            .enumerate()
            .flat_map(|(loc, inv)| {
                inv.iter().map(move |(dest, qty)| {
                    json!({ "location": loc, "destination": dest, "quantity": qty })
                })
            })
            .collect();
        out["cargo"] = Value::Array(docks);
    }
    out
}
