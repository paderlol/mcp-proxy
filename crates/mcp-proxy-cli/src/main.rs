//! `mcp-proxy` CLI — wraps MCP server invocations with transparent secret injection.
//!
//! Invoked by AI clients (Claude Desktop, Codex, Cursor, etc.) as configured in
//! their respective config files:
//!   { "command": "mcp-proxy", "args": ["run", "<server-id>"] }
//!
//! Reads server + secret metadata from the same JSON store as the Tauri desktop app,
//! resolves secrets from their backends (Keychain / 1Password / EncryptedFile),
//! spawns the real MCP server with env vars injected, and uses inherited stdio
//! so MCP protocol traffic flows transparently.

mod docker;

use clap::{Parser, Subcommand};
use mcp_proxy_common::models::{McpServerConfig, RunMode, SecretMeta};
use mcp_proxy_common::secret_resolver::resolve_secret;
use mcp_proxy_common::store::{app_data_dir, load_json, secrets_meta_path, servers_path};
use std::collections::HashMap;
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "mcp-proxy")]
#[command(
    about = "MCP Proxy — wrap MCP servers with secure secret injection",
    long_about = "A CLI wrapper that resolves secrets from local storage (Keychain on \
                  macOS, encrypted vault on other platforms) or from 1Password, injects \
                  them as environment variables, and transparently proxies stdio to the \
                  real MCP server process."
)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run an MCP server with secrets resolved and injected as env vars
    Run {
        /// Server ID as configured in the MCP Proxy desktop app
        server_id: String,
    },
    /// List all configured MCP servers
    List,
}

fn main() {
    // Keep logging off stdout — stdio is reserved for MCP protocol traffic
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Run { server_id } => run_server(&server_id),
        Commands::List => list_servers(),
    };

    if let Err(e) = result {
        eprintln!("mcp-proxy: {e}");
        std::process::exit(1);
    }
}

fn run_server(server_id: &str) -> Result<(), String> {
    // 1. Load server configs
    let servers: Vec<McpServerConfig> = load_json(servers_path()).ok_or_else(|| {
        format!(
            "No servers.json found at {}. \
             Add a server via the MCP Proxy desktop app first.",
            servers_path().display()
        )
    })?;

    let config = servers
        .iter()
        .find(|s| s.id == server_id)
        .cloned()
        .ok_or_else(|| format!("Server '{server_id}' not found in servers.json"))?;

    if !config.enabled {
        return Err(format!("Server '{server_id}' is disabled"));
    }

    // 2. Load secret metadata
    let secret_metas: Vec<SecretMeta> = load_json(secrets_meta_path()).unwrap_or_default();

    // 3. Resolve env vars from secrets (uses tokio for async resolvers)
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;

    let mut env_vars: HashMap<String, String> = HashMap::new();
    for mapping in &config.env_mappings {
        let meta = secret_metas
            .iter()
            .find(|m| m.id == mapping.secret_ref)
            .ok_or_else(|| {
                format!(
                    "Secret '{}' referenced by env var '{}' not found",
                    mapping.secret_ref, mapping.env_var_name
                )
            })?;

        tracing::debug!(
            "Resolving secret '{}' for env var '{}'",
            meta.id,
            mapping.env_var_name
        );

        let value = runtime.block_on(resolve_secret(&meta.id, &meta.source))?;
        env_vars.insert(mapping.env_var_name.clone(), value);
    }

    tracing::info!(
        "Launching server '{}' (command: {}, {} env vars)",
        config.name,
        config.command,
        env_vars.len()
    );

    // 4. Dispatch based on run mode
    match &config.run_mode {
        RunMode::Local => spawn_local(&config, env_vars),
        RunMode::DockerSandbox { image, extra_args } => {
            let image = image.as_deref().ok_or_else(|| {
                "Docker sandbox requires a base image — edit the server config and set one \
                 (e.g., `node:20-alpine` for npx-based servers)."
                    .to_string()
            })?;
            let build_root = app_data_dir().join("docker-build");
            docker::run_sandbox(docker::SandboxConfig {
                server_id: &config.id,
                image,
                command: &config.command,
                args: &config.args,
                env_vars: &env_vars,
                extra_args,
                build_root: &build_root,
            })
        }
    }
}

fn spawn_local(
    config: &McpServerConfig,
    env_vars: HashMap<String, String>,
) -> Result<(), String> {
    // Inherit stdio: AI client's stdin/stdout IS our stdin/stdout,
    // and the child inherits them directly. MCP protocol traffic flows through
    // without any manual piping on our side.
    let mut child = Command::new(&config.command)
        .args(&config.args)
        .envs(&env_vars)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("Failed to spawn '{}': {e}", config.command))?;

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for child process: {e}"))?;

    if !status.success() {
        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }

    Ok(())
}

fn list_servers() -> Result<(), String> {
    let servers: Vec<McpServerConfig> = load_json(servers_path()).unwrap_or_default();

    if servers.is_empty() {
        println!("No MCP servers configured.");
        println!(
            "Add servers via the MCP Proxy desktop app, then they'll appear here."
        );
        return Ok(());
    }

    println!("Configured MCP servers:\n");
    for s in &servers {
        let status = if s.enabled { "enabled" } else { "disabled" };
        let mode = match &s.run_mode {
            RunMode::Local => "local",
            RunMode::DockerSandbox { .. } => "docker",
        };
        println!("  {}  ({}, {})", s.id, status, mode);
        println!("    name:    {}", s.name);
        println!("    command: {} {}", s.command, s.args.join(" "));
        if !s.env_mappings.is_empty() {
            println!("    env vars:");
            for m in &s.env_mappings {
                println!("      {} → secret:{}", m.env_var_name, m.secret_ref);
            }
        }
        println!();
    }
    Ok(())
}
