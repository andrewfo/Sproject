//! Plain world-state data. No validation or rules live here.

use indexmap::IndexMap;

pub type VesselId = String;
pub type Location = usize;
pub type Tick = u64;

/// Cargo keyed by destination, in insertion order (the spec's output order).
pub type Inventory = IndexMap<Location, u32>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VesselStatus {
    Idle {
        location: Location,
    },
    Docked {
        location: Location,
    },
    Transit {
        from: Location,
        to: Location,
        arrives_at_tick: Tick,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vessel {
    pub status: VesselStatus,
    pub cargo: Inventory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub tick: Tick,
    /// Init order is preserved so output matches the spec examples.
    pub vessels: IndexMap<VesselId, Vessel>,
    /// One inventory per location.
    pub docks: Vec<Inventory>,
}

/// Static configuration fixed at `init`.
#[derive(Debug)]
pub struct Config {
    pub travel_times: Vec<Vec<u64>>,
    /// `None` means base mode: no docks or cargo, and no `cargo` fields in output.
    pub dock_throughput: Option<Vec<u32>>,
}

impl Config {
    pub fn num_locations(&self) -> usize {
        self.travel_times.len()
    }

    pub fn cargo_enabled(&self) -> bool {
        self.dock_throughput.is_some()
    }
}

/// Adds `qty` units for `dest` to an inventory.
pub fn add_cargo(inv: &mut Inventory, dest: Location, qty: u32) {
    *inv.entry(dest).or_insert(0) += qty;
}

/// Removes `qty` units for `dest`. Returns false (and leaves `inv` untouched) if there aren't enough.
pub fn remove_cargo(inv: &mut Inventory, dest: Location, qty: u32) -> bool {
    match inv.get_mut(&dest) {
        Some(have) if *have >= qty => {
            *have -= qty;
            if *have == 0 {
                inv.shift_remove(&dest);
            }
            true
        }
        _ => false,
    }
}
