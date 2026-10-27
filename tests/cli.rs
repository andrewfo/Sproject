//! Runs the real binary, for the stdin/stdout handling that `Engine` tests can't reach.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;

fn run_raw(input: &[u8]) -> (String, bool) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fleet-sim"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let out = child.wait_with_output().unwrap();
    (String::from_utf8(out.stdout).unwrap(), out.status.success())
}

fn run(input: &[u8]) -> (Vec<Value>, bool) {
    let (stdout, ok) = run_raw(input);
    let lines = stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (lines, ok)
}

#[test]
fn invalid_utf8_line_gets_an_error_and_the_loop_keeps_going() {
    let mut input =
        br#"{"command":"init","parameters":{"travel_times":[[0]],"vessels":[]}}"#.to_vec();
    input.extend_from_slice(b"\n\xff\xfe\n");
    input.extend_from_slice(br#"{"command":"get_state","parameters":{}}"#);
    input.push(b'\n');

    let (lines, ok) = run(&input);
    assert!(ok);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["status"], "ok");
    assert_eq!(
        lines[1]["payload"]["message"],
        "invalid command: line is not valid UTF-8"
    );
    assert_eq!(lines[2]["status"], "ok");
}

#[test]
fn blank_and_crlf_lines_are_handled() {
    let input = b"\r\n   \n{\"command\":\"get_state\",\"parameters\":{}}\r\n";
    let (lines, ok) = run(input);
    assert!(ok);
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0]["payload"]["message"],
        "simulation not initialized; send init first"
    );
}

/// The golden tests compare parsed JSON, so they'd miss a change in key order or spacing. This
/// checks the binary's raw stdout against every fixture byte for byte.
#[test]
fn golden_outputs_match_byte_for_byte() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            name.strip_suffix(".in.jsonl").map(str::to_owned)
        })
        .collect();
    names.sort();
    assert!(!names.is_empty());
    for name in names {
        let input = std::fs::read(dir.join(format!("{name}.in.jsonl"))).unwrap();
        let want = std::fs::read_to_string(dir.join(format!("{name}.out.jsonl"))).unwrap();

        let (got, ok) = run_raw(&input);
        assert!(ok);

        for (i, (got, want)) in got.lines().zip(want.lines()).enumerate() {
            assert_eq!(got, want, "{name} line {}", i + 1);
        }
        assert_eq!(
            got.lines().count(),
            want.lines().count(),
            "{name} line count"
        );
    }
}
