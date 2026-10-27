//! Property tests: random command streams (valid and invalid) against a cargo-mode sim, checking
//! the things that should hold no matter what gets thrown at it.

use std::collections::HashMap;

use fleet_sim::Engine;
use proptest::prelude::*;
use serde_json::{Value, json};

const N: u64 = 3;
const VESSELS: [&str; 3] = ["v1", "v2", "v3"];

fn init_line() -> String {
    json!({"command": "init", "parameters": {
        "travel_times": [[0, 2, 3], [2, 0, 1], [3, 1, 0]],
        "dock_throughput": [2, 1, 3],
        "vessels": [{"id": "v1", "location": 0}, {"id": "v2", "location": 0}, {"id": "v3", "location": 2}],
        "cargo_manifest": [{"location": 0, "destination": 1, "quantity": 2}, {"location": 2, "destination": 0, "quantity": 1}],
    }})
    .to_string()
}

#[derive(Debug, Clone)]
enum Cmd {
    Tick(Vec<(u64, u64, u32)>),
    SetDestination(usize, u64),
    Dock(usize),
    Undock(usize),
    Load(usize, u64),
    Unload(usize, u64),
    Swap(usize, u64, u64),
    GetState,
    GetStateAt(i64),
    Garbage,
}

/// Locations and vessels go one past the valid range so the error paths get hit too.
fn cmd() -> impl Strategy<Value = Cmd> {
    let vessel = 0..=VESSELS.len();
    let loc = || 0..=N;
    prop_oneof![
        3 => prop::collection::vec((loc(), loc(), 0..3u32), 0..3).prop_map(Cmd::Tick),
        2 => (vessel.clone(), loc()).prop_map(|(v, d)| Cmd::SetDestination(v, d)),
        2 => vessel.clone().prop_map(Cmd::Dock),
        1 => vessel.clone().prop_map(Cmd::Undock),
        3 => (vessel.clone(), loc()).prop_map(|(v, d)| Cmd::Load(v, d)),
        3 => (vessel.clone(), loc()).prop_map(|(v, d)| Cmd::Unload(v, d)),
        2 => (vessel, loc(), loc()).prop_map(|(v, l, u)| Cmd::Swap(v, l, u)),
        1 => Just(Cmd::GetState),
        1 => (-1..30i64).prop_map(Cmd::GetStateAt),
        1 => Just(Cmd::Garbage),
    ]
}

fn vessel_id(i: usize) -> &'static str {
    VESSELS.get(i).copied().unwrap_or("nope")
}

fn line(cmd: &Cmd) -> String {
    let c = |name: &str, params: Value| json!({"command": name, "parameters": params}).to_string();
    match cmd {
        Cmd::Tick(manifest) => c(
            "tick",
            json!({"cargo_manifest": manifest.iter().map(|&(l, d, q)| json!({"location": l, "destination": d, "quantity": q})).collect::<Vec<_>>()}),
        ),
        Cmd::SetDestination(v, d) => c(
            "set_destination",
            json!({"vessel_id": vessel_id(*v), "destination": d}),
        ),
        Cmd::Dock(v) => c("dock", json!({"vessel_id": vessel_id(*v)})),
        Cmd::Undock(v) => c("undock", json!({"vessel_id": vessel_id(*v)})),
        Cmd::Load(v, d) => c(
            "load",
            json!({"vessel_id": vessel_id(*v), "destination": d}),
        ),
        Cmd::Unload(v, d) => c(
            "unload",
            json!({"vessel_id": vessel_id(*v), "destination": d}),
        ),
        Cmd::Swap(v, l, u) => c(
            "swap",
            json!({"vessel_id": vessel_id(*v), "load_destination": l, "unload_destination": u}),
        ),
        Cmd::GetState => c("get_state", json!({})),
        Cmd::GetStateAt(t) => c("get_state_at", json!({"tick": t})),
        Cmd::Garbage => "{\"command\":".to_owned(),
    }
}

fn send(e: &mut Engine, line: &str) -> (bool, Value) {
    let out = e.handle_line(line);
    assert!(!out.contains('\n'), "response spans more than one line");
    let v: Value = serde_json::from_str(&out).expect("response is JSON");
    (v["status"] == "ok", v["payload"].clone())
}

fn total_cargo(world: &Value) -> u64 {
    let sum = |items: &Value| -> u64 {
        items
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["quantity"].as_u64().unwrap())
            .sum()
    };
    let on_vessels: u64 = world["vessels"]
        .as_object()
        .unwrap()
        .values()
        .map(|v| sum(&v["cargo"]))
        .sum();
    sum(&world["cargo"]) + on_vessels
}

/// Where a vessel is docked in a rendered world, if it is.
fn docked_at(world: &Value, vessel: &str) -> Option<u64> {
    let v = &world["vessels"][vessel];
    (v["state"] == "docked").then(|| v["location"].as_u64().unwrap())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn random_command_streams_keep_the_invariants(cmds in prop::collection::vec(cmd(), 0..80)) {
        let mut e = Engine::new();
        let (ok, mut planned) = send(&mut e, &init_line());
        prop_assert!(ok);

        // cargo that should exist right now: everything manifested minus everything delivered
        let mut expected_total = total_cargo(&planned);
        // what each tick looked like when it was committed
        let mut snapshots: HashMap<u64, Value> = HashMap::from([(0, planned.clone())]);
        let mut current = 0u64;

        for cmd in &cmds {
            let (ok, payload) = send(&mut e, &line(cmd));
            if !ok {
                prop_assert!(payload["message"].is_string());
                continue;
            }
            match cmd {
                Cmd::GetStateAt(t) => {
                    prop_assert_eq!(&payload, &snapshots[&(*t as u64)], "history[{}] changed", t);
                    continue;
                }
                Cmd::Tick(manifest) => {
                    expected_total += manifest.iter().map(|&(_, _, q)| u64::from(q)).sum::<u64>();
                    current += 1;
                    prop_assert_eq!(payload["tick"].as_u64(), Some(current));
                    // right after a tick, planned and committed agree
                    let (_, committed) = send(&mut e, &line(&Cmd::GetStateAt(current as i64)));
                    prop_assert_eq!(&committed, &payload);
                    snapshots.insert(current, payload.clone());
                }
                Cmd::Unload(v, d) | Cmd::Swap(v, _, d)
                    if docked_at(&planned, vessel_id(*v)) == Some(*d) =>
                {
                    expected_total -= 1;
                }
                _ => {}
            }
            prop_assert_eq!(total_cargo(&payload), expected_total, "cargo not conserved after {:?}", cmd);
            planned = payload;
        }

        // every tick still reads back the same as when it was committed
        for (t, snapshot) in &snapshots {
            let (ok, payload) = send(&mut e, &line(&Cmd::GetStateAt(*t as i64)));
            prop_assert!(ok);
            prop_assert_eq!(&payload, snapshot);
        }
    }
}
