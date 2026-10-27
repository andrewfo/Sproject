//! Golden tests: each `tests/golden/NAME.in.jsonl` is fed line by line to a fresh engine and every
//! response is compared against the matching line of `NAME.out.jsonl`.
//! Comparison uses parsed JSON, so object key order doesn't matter but array order does.

use std::fs;
use std::path::Path;

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

#[test]
#[ignore = "engine not implemented yet"]
fn golden_files() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut cases: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().ends_with(".in.jsonl"))
        .collect();
    cases.sort();
    assert!(!cases.is_empty(), "no golden cases in {}", dir.display());
    for case in cases {
        run_case(&case);
    }
}
