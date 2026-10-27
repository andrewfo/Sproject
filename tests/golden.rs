//! Golden tests. Each `tests/golden/NAME.in.jsonl` gets fed line by line into a fresh engine and
//! each response is checked against the same line in `NAME.out.jsonl`. Lines are compared as
//! parsed JSON, so key order doesn't matter (array order still does).
//!
//! One test per fixture so they can be turned on one at a time. If you add a fixture and forget
//! to put it in `golden!` below, `all_fixtures_registered` fails.

use std::fs;
use std::path::Path;

use fleet_sim::Engine;
use serde_json::Value;

fn run_case(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let read = |ext: &str| {
        let path = dir.join(format!("{name}.{ext}.jsonl"));
        fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing {}", path.display()))
    };
    let (inputs, expected) = (read("in"), read("out"));

    let inputs: Vec<&str> = inputs.lines().filter(|l| !l.trim().is_empty()).collect();
    let expected: Vec<&str> = expected.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        inputs.len(),
        expected.len(),
        "{name}: input/output line counts differ"
    );

    let mut engine = Engine::new();
    for (i, (cmd, want)) in inputs.iter().zip(&expected).enumerate() {
        let got: Value = serde_json::from_str(&engine.handle_line(cmd)).unwrap();
        let want: Value = serde_json::from_str(want).unwrap();
        assert_eq!(
            got,
            want,
            "{name} line {}\n  cmd:  {cmd}\n  got:  {got}\n  want: {want}",
            i + 1
        );
    }
}

macro_rules! golden {
    ($($(#[$attr:meta])* $name:ident),* $(,)?) => {
        const REGISTERED: &[&str] = &[$(stringify!($name)),*];
        $(
            #[test]
            $(#[$attr])*
            fn $name() {
                run_case(stringify!($name));
            }
        )*
    };
}

golden! {
    base_example,
    #[ignore = "engine not implemented yet"]
    cargo_example,
    #[ignore = "engine not implemented yet"]
    history_example,
}

#[test]
fn all_fixtures_registered() {
    let mut unregistered: Vec<String> =
        fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden"))
            .unwrap()
            .filter_map(|e| {
                let name = e.unwrap().file_name().into_string().unwrap();
                name.strip_suffix(".in.jsonl").map(str::to_owned)
            })
            .filter(|name| !REGISTERED.contains(&name.as_str()))
            .collect();
    unregistered.sort();
    assert!(
        unregistered.is_empty(),
        "fixtures missing from golden!: {unregistered:?}"
    );
}
