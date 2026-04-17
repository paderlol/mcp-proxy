//! End-to-end tests proving the CLI's stdio transport works correctly.
//!
//! The MCP protocol is JSON-RPC 2.0 over stdio. If `mcp-proxy run` correctly
//! inherits stdio to the child process, any MCP server works through it —
//! regardless of which AI client (Claude Desktop, Codex, Cursor, …) is on the
//! other side. These tests use `/bin/cat` and `/bin/sh` as stand-in "servers"
//! so the assertion is purely about byte-level fidelity.

#![cfg(unix)]

use serde_json::json;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn write_servers(data_dir: &Path, value: serde_json::Value) {
    fs::write(
        data_dir.join("servers.json"),
        serde_json::to_string_pretty(&value).unwrap(),
    )
    .unwrap();
}

fn cli_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mcp-proxy")
}

/// Proves the critical invariant: bytes written to `mcp-proxy run`'s stdin
/// reach the child process unchanged, and the child's stdout reaches
/// `mcp-proxy run`'s stdout unchanged. This is what makes the MCP protocol
/// work transparently.
#[test]
fn stdio_is_transparently_piped_to_child() {
    let tmp = TempDir::new().unwrap();

    write_servers(
        tmp.path(),
        json!([{
            "id": "cat-echo",
            "name": "Cat Echo",
            "command": "/bin/cat",
            "args": [],
            "transport": { "type": "Stdio" },
            "env_mappings": [],
            "run_mode": { "type": "Local" },
            "enabled": true,
            "trusted": false,
            "created_at": "2026-04-17T00:00:00Z",
            "updated_at": "2026-04-17T00:00:00Z",
        }]),
    );

    let mut child = Command::new(cli_bin())
        .env("MCP_PROXY_DATA_DIR", tmp.path())
        .env("RUST_LOG", "off")
        .args(["run", "cat-echo"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn mcp-proxy");

    // A realistic MCP initialize request — the very first message any AI
    // client sends. If this round-trips through cat, real clients will work.
    let initialize_request = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test-harness","version":"0.0.0"}}}"#;

    {
        let stdin = child.stdin.as_mut().expect("stdin handle");
        writeln!(stdin, "{initialize_request}").unwrap();
    }
    // Drop stdin so `cat` hits EOF and exits.
    drop(child.stdin.take());

    let mut stdout = String::new();
    child
        .stdout
        .as_mut()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();

    let status = child.wait().expect("wait on mcp-proxy");

    assert!(status.success(), "mcp-proxy exited non-zero; stderr: {}", {
        let mut s = String::new();
        if let Some(mut err) = child.stderr.take() {
            err.read_to_string(&mut s).ok();
        }
        s
    });
    assert_eq!(
        stdout.trim(),
        initialize_request,
        "cat should echo the JSON-RPC payload byte-for-byte"
    );
}

/// Proves the child inherits the parent's environment — `PATH`, `HOME`, etc.
/// must flow through for tools like `npx`, `uvx`, `node` to work.
#[test]
fn child_sees_inherited_env_vars() {
    let tmp = TempDir::new().unwrap();

    write_servers(
        tmp.path(),
        json!([{
            "id": "env-echo",
            "name": "Env Echo",
            "command": "/bin/sh",
            // ${PATH:+yes} is POSIX — prints "yes" iff PATH is non-empty.
            "args": ["-c", "printf 'PATH_PRESENT=%s\\n' \"${PATH:+yes}\""],
            "transport": { "type": "Stdio" },
            "env_mappings": [],
            "run_mode": { "type": "Local" },
            "enabled": true,
            "trusted": false,
            "created_at": "2026-04-17T00:00:00Z",
            "updated_at": "2026-04-17T00:00:00Z",
        }]),
    );

    let output = Command::new(cli_bin())
        .env("MCP_PROXY_DATA_DIR", tmp.path())
        .env("RUST_LOG", "off")
        .args(["run", "env-echo"])
        .output()
        .expect("spawn mcp-proxy");

    assert!(
        output.status.success(),
        "mcp-proxy exited non-zero; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("PATH_PRESENT=yes"),
        "child did not inherit PATH; got stdout: {stdout:?}"
    );
}
