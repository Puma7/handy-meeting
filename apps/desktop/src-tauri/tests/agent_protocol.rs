//! Actual executable/stdio boundaries; no window, model or audio device is opened.
use serde_json::{json, Value};
use std::{
    io::Write,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn run(arguments: &[&str], input: &[u8]) -> (bool, Vec<u8>, Vec<u8>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_HandyMeeting"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Owned model-free subprocess exceeded its deadline");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output().unwrap();
    (output.status.success(), output.stdout, output.stderr)
}
fn rows(output: &[u8]) -> Vec<Value> {
    output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            assert!(line.len() <= 65536);
            serde_json::from_slice(line).expect("stdout contains only JSON-RPC responses")
        })
        .collect()
}
#[test]
fn executable_protocol_has_no_implicit_consent_paths_or_cloud_authority() {
    let input=[
        json!({"jsonrpc":"2.0","id":1,"method":"capabilities","params":{}}),
        json!({"jsonrpc":"2.0","id":2,"method":"transcript.updates","params":{"sessionId":"foreign","afterSequence":0,"limit":1}}),
        json!({"jsonrpc":"2.0","id":3,"method":"request_recording","params":{"title":"Synthetic protocol only","includeMicrophone":false,"maxDurationSeconds":60}}),
        json!({"jsonrpc":"2.0","id":4,"method":"approve","params":{}}),
        json!({"jsonrpc":"2.0","id":5,"method":"notes.generate","params":{}}),
        json!({"jsonrpc":"2.0","id":6,"method":"status","params":{}}),
    ].map(|value|serde_json::to_string(&value).unwrap()).join("\n")+"\n";
    let (success, out, err) = run(&["--agent-protocol-check-stdio"], input.as_bytes());
    assert!(success, "{}", String::from_utf8_lossy(&err));
    assert!(err.is_empty());
    let replies = rows(&out);
    assert_eq!(replies.len(), 6);
    assert_eq!(replies[0]["result"]["cloudExecution"], false);
    assert_eq!(replies[0]["result"]["mcp"], false);
    assert!(replies[1].get("error").is_some());
    assert_eq!(replies[2]["result"]["recordingStarted"], false);
    assert!(replies[3].get("error").is_some());
    assert!(replies[4].get("error").is_some());
    assert!(replies[5]["result"]["ownedSessionId"].is_null());
}
#[test]
fn executable_rejects_malformed_duplicate_oversized_and_cross_protocol_requests() {
    let input=b"{bad}\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"connect\",\"params\":{\"label\":\"first\",\"label\":\"duplicate\"}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"Feed\",\"params\":{}}\n";
    let (success, out, err) = run(&["--agent-protocol-check-stdio"], input);
    assert!(success);
    assert!(err.is_empty());
    let replies = rows(&out);
    assert_eq!(replies.len(), 3);
    assert!(replies.iter().all(|reply| reply.get("error").is_some()));
    let (success, out, err) = run(&["--agent-protocol-check-stdio"], &vec![b'x'; 65537]);
    assert!(success);
    assert!(err.is_empty());
    let replies = rows(&out);
    assert_eq!(replies.len(), 1);
    assert!(replies[0].get("error").is_some());
}
#[test]
fn capability_modes_fail_closed_before_handy_discovery_and_never_open_a_window() {
    let (success, out, err) = run(&["--agent", "capabilities"], b"");
    assert!(success);
    assert!(err.is_empty());
    assert_eq!(rows(&out)[0]["protocol"], "handymeeting-agent-v2");
    for args in [
        vec!["--agent", "serve", "--allow-recording"],
        vec!["--companion-capabilities", "--contract-version", "2"],
        vec!["--companion-capabilities", "--unknown", "1"],
        vec!["--companion-capabilities", "--contract-version"],
    ] {
        let (success, out, err) = run(&args, b"");
        assert!(!success);
        assert!(out.is_empty());
        assert!(!err.is_empty());
    }
}
