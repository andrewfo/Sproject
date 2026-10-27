//! JSON in and out: parsing commands and rendering responses.

use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};

use crate::error::SimError;
use crate::state::{Inventory, Location, Vessel, VesselId, VesselStatus, World};

#[derive(Debug, Deserialize)]
#[serde(
    tag = "command",
    content = "parameters",
    rename_all = "snake_case",
    deny_unknown_fields
)]
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
        // Signed so a negative tick gets a clean error instead of a parse failure, and wider than
        // u64 so any tick past u64::MAX is still just "in the future".
        tick: i128,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitParams {
    pub travel_times: Vec<Vec<u64>>,
    pub vessels: Vec<VesselSpec>,
    #[serde(default)]
    pub dock_throughput: Option<Vec<u32>>,
    #[serde(default)]
    pub cargo_manifest: Vec<ManifestEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VesselSpec {
    pub id: VesselId,
    pub location: Location,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TickParams {
    #[serde(default)]
    pub cargo_manifest: Vec<ManifestEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestEntry {
    pub location: Location,
    pub destination: Location,
    pub quantity: u32,
}

pub fn parse_command(line: &str) -> Result<Command, SimError> {
    serde_json::from_str(line).map_err(|e| SimError::InvalidCommand(e.to_string()))
}

/// Wraps a command result in the spec's response envelope and serializes it to one line.
pub fn render_response(result: Result<WorldView<'_>, SimError>) -> String {
    let line = match result {
        Ok(world) => serde_json::to_string(&Envelope {
            status: "ok",
            payload: world,
        }),
        Err(err) => serde_json::to_string(&Envelope {
            status: "error",
            payload: ErrorPayload {
                message: err.to_string(),
            },
        }),
    };
    line.expect("responses only hold strings, integers, and string-keyed maps")
}

#[derive(Serialize)]
struct Envelope<T> {
    status: &'static str,
    payload: T,
}

#[derive(Serialize)]
struct ErrorPayload {
    message: String,
}

/// A borrowed view of a world that serializes straight to the spec's `WorldState` JSON, with no
/// intermediate `Value` tree. `cargo` fields appear only when cargo is enabled.
#[derive(Clone, Copy)]
pub struct WorldView<'a> {
    pub world: &'a World,
    pub cargo_enabled: bool,
}

impl Serialize for WorldView<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("tick", &self.world.tick)?;
        map.serialize_entry("vessels", &Vessels(*self))?;
        if self.cargo_enabled {
            map.serialize_entry("cargo", &DockCargo(&self.world.docks))?;
        }
        map.end()
    }
}

struct Vessels<'a>(WorldView<'a>);

impl Serialize for Vessels<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let cargo_enabled = self.0.cargo_enabled;
        s.collect_map(self.0.world.vessels.iter().map(|(id, vessel)| {
            (
                id,
                VesselView {
                    vessel,
                    cargo_enabled,
                },
            )
        }))
    }
}

struct VesselView<'a> {
    vessel: &'a Vessel,
    cargo_enabled: bool,
}

impl Serialize for VesselView<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        match self.vessel.status {
            VesselStatus::Idle { location } => {
                map.serialize_entry("state", "idle")?;
                map.serialize_entry("location", &location)?;
            }
            VesselStatus::Docked { location } => {
                map.serialize_entry("state", "docked")?;
                map.serialize_entry("location", &location)?;
            }
            VesselStatus::Transit {
                from,
                to,
                arrives_at_tick,
            } => {
                map.serialize_entry("state", "transit")?;
                map.serialize_entry("from", &from)?;
                map.serialize_entry("to", &to)?;
                map.serialize_entry("arrives_at_tick", &arrives_at_tick)?;
            }
        }
        if self.cargo_enabled {
            map.serialize_entry("cargo", &VesselCargo(&self.vessel.cargo))?;
        }
        map.end()
    }
}

struct VesselCargo<'a>(&'a Inventory);

impl Serialize for VesselCargo<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(|(&destination, &quantity)| CargoEntry {
            destination,
            quantity,
        }))
    }
}

#[derive(Serialize)]
struct CargoEntry {
    destination: Location,
    quantity: u32,
}

/// Dock cargo as one flat list, ordered by location and then insertion order.
struct DockCargo<'a>(&'a [Inventory]);

impl Serialize for DockCargo<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().enumerate().flat_map(|(location, inv)| {
            inv.iter()
                .map(move |(&destination, &quantity)| DockCargoEntry {
                    location,
                    destination,
                    quantity,
                })
        }))
    }
}

#[derive(Serialize)]
struct DockCargoEntry {
    location: Location,
    destination: Location,
    quantity: u32,
}
