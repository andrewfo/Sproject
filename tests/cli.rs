//! Runs the real binary, for the stdin/stdout handling that `Engine` tests can't reach.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;

fn run(input: &[u8]) -> (Vec<Value>, bool) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fleet-sim"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let out = child.wait_with_output().unwrap();
    let lines = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (lines, out.status.success())
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
