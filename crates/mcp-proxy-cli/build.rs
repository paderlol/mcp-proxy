//! Compute a sha256 of the embedded agent source at CLI build time and expose
//! it as `MCP_PROXY_AGENT_SRC_SHA256`. The runtime Docker-sandbox path rehashes
//! the same embedded bytes and refuses to build an image if the two disagree —
//! a cheap integrity check that catches in-flight tampering with the bytes the
//! `include_str!` macro captured.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

fn main() {
    let cargo = Path::new("../mcp-proxy-agent/Cargo.toml");
    let main_rs = Path::new("../mcp-proxy-agent/src/main.rs");

    let cargo_bytes = fs::read(cargo).expect("read agent Cargo.toml");
    let main_bytes = fs::read(main_rs).expect("read agent main.rs");

    let mut h = Sha256::new();
    h.update(&cargo_bytes);
    h.update(&main_bytes);
    let hex: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();

    println!("cargo:rustc-env=MCP_PROXY_AGENT_SRC_SHA256={hex}");
    println!("cargo:rerun-if-changed={}", cargo.display());
    println!("cargo:rerun-if-changed={}", main_rs.display());
}
