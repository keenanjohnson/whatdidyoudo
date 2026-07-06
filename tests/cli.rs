//! End-to-end CLI: run the built `wdyd` binary against fixtures and check its
//! output and exit codes. `CARGO_BIN_EXE_wdyd` is set by cargo for integration tests.

use std::process::Command;

fn wdyd(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_wdyd"))
        .args(args)
        .output()
        .expect("run wdyd")
}

fn fixture(name: &str) -> String {
    format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn json_output_reports_two_contradictions() {
    let out = wdyd(&[
        "--session",
        &fixture("session_typical.jsonl"),
        "--format",
        "json",
    ]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid json");
    assert_eq!(v["trust"]["contradicted"], 2);
    assert_eq!(v["trust"]["verified"], 1);
    assert_eq!(v["claims"].as_array().unwrap().len(), 3);
}

#[test]
fn markdown_output_is_pr_ready() {
    let out = wdyd(&[
        "--session",
        &fixture("session_typical.jsonl"),
        "--format",
        "md",
    ]);
    let md = String::from_utf8_lossy(&out.stdout);
    assert!(md.contains("## what did you do?"));
    assert!(md.contains("| build succeeds | ❌ Contradicted |"));
}

#[test]
fn check_exits_nonzero_on_contradiction() {
    let out = wdyd(&["--session", &fixture("session_typical.jsonl"), "--check"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn check_exits_zero_when_nothing_contradicted() {
    // The noise session has no claims, so nothing can be contradicted.
    let out = wdyd(&["--session", &fixture("session_noise.jsonl"), "--check"]);
    assert!(out.status.success());
}
