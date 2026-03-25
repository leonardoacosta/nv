//! Integration smoke tests for the nv-tools MCP server.
//!
//! These tests spawn the `nv-tools` binary as a subprocess, drive the
//! JSON-RPC protocol over stdin/stdout, and assert the server behaves
//! correctly.
//!
//! Run with:
//!   cargo test -p nv-tools --features integration
//!
//! The binary must be pre-built:
//!   cargo build -p nv-tools

#![cfg(feature = "integration")]

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Locate the nv-tools binary.  We prefer the release build if it exists,
/// otherwise fall back to the debug build so CI can run without `--release`.
fn binary_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // CARGO_MANIFEST_DIR is crates/nv-tools; workspace root is two levels up
    let workspace_root = manifest_dir
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root");

    let release = workspace_root.join("target/release/nv-tools");
    let debug = workspace_root.join("target/debug/nv-tools");

    if release.exists() {
        release
    } else if debug.exists() {
        debug
    } else {
        panic!(
            "nv-tools binary not found at {} or {}.\n\
             Build it first: cargo build -p nv-tools",
            release.display(),
            debug.display()
        );
    }
}

/// Spawn the server and return the child + line-buffered reader on its stdout.
fn spawn_server() -> (Child, BufReader<std::process::ChildStdout>) {
    let bin = binary_path();
    let mut child = Command::new(&bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // Silence tracing output so it doesn't pollute the stdout we're parsing
        .env("RUST_LOG", "off")
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", bin.display()));

    let stdout = child.stdout.take().expect("stdout");
    let reader = BufReader::new(stdout);
    (child, reader)
}

/// Write one JSON-RPC line to the process stdin.
fn send(stdin: &mut std::process::ChildStdin, value: &serde_json::Value) {
    let mut line = serde_json::to_string(value).expect("serialize");
    line.push('\n');
    stdin.write_all(line.as_bytes()).expect("write stdin");
    stdin.flush().expect("flush stdin");
}

/// Read one JSON-RPC response line from stdout.
fn recv(reader: &mut BufReader<std::process::ChildStdout>) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).expect("read stdout");
    serde_json::from_str(line.trim()).unwrap_or_else(|e| {
        panic!("failed to parse response JSON: {e}\nRaw line: {line:?}")
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// [1.3] initialize — serverInfo.name must be "nv-tools"
#[test]
fn test_initialize() {
    let (mut child, mut reader) = spawn_server();
    let mut stdin = child.stdin.take().expect("stdin");

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "smoke-test", "version": "0.0.1" }
        }
    });
    send(&mut stdin, &request);
    let response = recv(&mut reader);

    assert_eq!(
        response["jsonrpc"],
        "2.0",
        "response must be JSON-RPC 2.0: {response}"
    );
    assert!(
        response.get("error").is_none(),
        "initialize must not return an error: {response}"
    );
    let server_name = response["result"]["serverInfo"]["name"]
        .as_str()
        .expect("serverInfo.name must be a string");
    assert_eq!(
        server_name, "nv-tools",
        "serverInfo.name must be 'nv-tools', got '{server_name}'"
    );

    drop(stdin);
    let _ = child.wait();
}

/// [1.4] tools/list — must return >= 40 tools
#[test]
fn test_tools_list() {
    let (mut child, mut reader) = spawn_server();
    let mut stdin = child.stdin.take().expect("stdin");

    // Initialize first (required by MCP protocol)
    send(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2024-11-05", "capabilities": {} }
        }),
    );
    let _ = recv(&mut reader);

    send(
        &mut stdin,
        &serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }),
    );
    let response = recv(&mut reader);

    assert!(
        response.get("error").is_none(),
        "tools/list must not return an error: {response}"
    );

    let tools = response["result"]["tools"]
        .as_array()
        .expect("result.tools must be an array");

    assert!(
        tools.len() >= 40,
        "expected >= 40 tools, got {} tools",
        tools.len()
    );

    drop(stdin);
    let _ = child.wait();
}

/// [1.5] tools/call for docker_status — must return a structured response.
///
/// If Docker is available: result contains tool output (not an error).
/// If Docker is unavailable: we accept a structured tool-level error embedded
/// in the result content, but NOT a JSON-RPC protocol error.
#[test]
fn test_docker_status_call() {
    let (mut child, mut reader) = spawn_server();
    let mut stdin = child.stdin.take().expect("stdin");

    // Initialize
    send(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2024-11-05", "capabilities": {} }
        }),
    );
    let _ = recv(&mut reader);

    // Call docker_status
    send(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "docker_status",
                "arguments": {}
            }
        }),
    );
    let response = recv(&mut reader);

    // A JSON-RPC protocol error (code -32601 "Method not found" or similar)
    // would indicate the server failed to route the call — that must never happen.
    if let Some(error) = response.get("error") {
        panic!(
            "docker_status returned a JSON-RPC protocol error (not a tool error): {error}\n\
             Full response: {response}"
        );
    }

    // There must be a result field
    assert!(
        response.get("result").is_some(),
        "docker_status response must have a 'result' field: {response}"
    );

    // The result may be a success or a tool-level error (isError: true with
    // content explaining Docker is unavailable) — both are acceptable.
    // We just verify the response is well-formed JSON-RPC.
    assert_eq!(
        response["jsonrpc"], "2.0",
        "response must be JSON-RPC 2.0: {response}"
    );

    drop(stdin);
    let _ = child.wait();
}
