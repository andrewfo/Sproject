//! Golden tests: each `tests/golden/NAME.in.jsonl` is fed line by line to a fresh engine and every
//! response is compared against the matching line of `NAME.out.jsonl`.
//! Comparison uses parsed JSON, so object key order doesn't matter but array order does.
//!
//! Each fixture gets its own test so cases can be enabled one at a time. A fixture that isn't
//! registered in `golden!` below fails `all_fixtures_registered`.

use std::fs;
use std::path::{Path, PathBuf};

use fleet_sim::Engine;
use serde_json::Value;

fn run_case(input: &Path) {
    let expected_path = input.with_file_name(
        input
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .replace(".in.jsonl", ".out.jsonl"),
    );
    let inputs = fs::read_to_string(input).unwrap();
    let expected = fs::read_to_string(&expected_path)
        .unwrap_or_else(|_| panic!("missing {}", expected_path.display()));

    let inputs: Vec<&str> = inputs.lines().filter(|l| !l.trim().is_empty()).collect();
    let expected: Vec<&str> = expected.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        inputs.len(),
        expected.len(),
        "{}: input/output line counts differ",
        input.display()
    );

    let mut engine = Engine::new();
    for (i, (cmd, want)) in inputs.iter().zip(&expected).enumerate() {
        let got: Value = serde_json::from_str(&engine.handle_line(cmd)).unwrap();
        let want: Value = serde_json::from_str(want).unwrap();
        assert_eq!(
            got,
            want,
            "{} line {}\n  cmd:  {cmd}\n  got:  {got}\n  want: {want}",
            input.display(),
            i + 1
        );
    }
}

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

macro_rules! golden {
    ($($(#[$attr:meta])* $name:ident),* $(,)?) => {
        const REGISTERED: &[&str] = &[$(stringify!($name)),*];
        $(
            #[test]
            $(#[$attr])*
            fn $name() {
                run_case(&golden_dir().join(concat!(stringify!($name), ".in.jsonl")));
            }
        )*
    };
}

golden! {
    #[ignore = "engine not implemented yet"]
    base_example,
    #[ignore = "engine not implemented yet"]
    cargo_example,
    #[ignore = "engine not implemented yet"]
    history_example,
}

#[test]
fn all_fixtures_registered() {
    let mut unregistered: Vec<String> = fs::read_dir(golden_dir())
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
