//! mcp-proxy-agent: runs inside Docker sandbox containers.
//!
//! Protocol:
//!   1. Reads exactly one line from stdin — a JSON payload containing secrets and the command to run
//!   2. Parses the payload, injects env vars, exec's the real MCP server
//!   3. All subsequent stdin/stdout is pure MCP protocol traffic (handled by the exec'd process)
//!
//! The host-side `mcp-proxy` writes the secret JSON as the first line, then bridges
//! the remaining stdin/stdout to the AI client.

use std::collections::HashMap;
use std::io::BufRead;
use std::os::unix::process::CommandExt;
use std::process::Command;

#[derive(serde::Deserialize)]
struct SecretPayload {
    env_vars: HashMap<String, String>,
    command: String,
    args: Vec<String>,
}

fn main() {
    // 1. Read exactly one line from stdin (the secret payload)
    let stdin = std::io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .expect("Failed to read secret payload from stdin");

    // 2. Parse the JSON payload
    let payload: SecretPayload =
        serde_json::from_str(line.trim()).expect("Failed to parse secret payload JSON from stdin");

    // 3. Build the command
    let mut cmd = Command::new(&payload.command);
    cmd.args(&payload.args);

    // 4. Inject secret env vars
    for (key, value) in &payload.env_vars {
        cmd.env(key, value);
    }

    // 5. Exec — replaces this process entirely.
    //    stdin/stdout are inherited, so subsequent MCP traffic flows through.
    let err = cmd.exec();
    eprintln!("mcp-proxy-agent: failed to exec {:?}: {err}", payload.command);
    std::process::exit(1);
}
