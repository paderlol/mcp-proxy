//! macOS `sandbox-exec` wrapper for Local-mode MCP server children.
//!
//! # Why
//!
//! Local run mode spawns an MCP server as a direct child of the CLI — fast and
//! zero-overhead, but the child inherits the user's full filesystem and network
//! access. Docker sandbox mode is the isolated alternative, but it has a
//! ~2-minute first-build cost and forces the user to pick a base image.
//!
//! `sandbox-exec(1)` is the only zero-dependency sandbox primitive shipped with
//! macOS. It is marked deprecated in the man page but Apple still relies on it
//! internally (Safari content processes, Xcode's test runner, etc.) and it is
//! present on every supported macOS version. We accept the deprecation risk as
//! the best-available middle ground.
//!
//! # What this module does
//!
//! When a server has `sandbox_local = true` *and* we're on macOS:
//!
//! 1. Generate a `.sb` (Scheme-like) profile string describing what the child
//!    may read / write / execute, and whether it may open network sockets.
//! 2. Write it to a per-run temp file (cleaned up with a [`TempProfile`] RAII
//!    guard when the caller is done waiting on the child).
//! 3. Return a `Command` that runs `sandbox-exec -f <profile> <cmd> <args...>`
//!    instead of `<cmd> <args...>` directly.
//!
//! On Linux / Windows every public function here is compiled out and the
//! caller falls back to a direct spawn.
//!
//! # Design choices (err toward too-permissive)
//!
//! The profile starts with `(deny default)` and layers specific allows on top:
//!
//! - **Read**: broad `/` read with a deny-list for obvious secret stores
//!   (`~/.ssh`, `~/Library/Keychains`, `~/.aws`, `~/.config/gh`, `/private/etc`
//!   shadow files). Narrower read-allow lists break legitimate MCP servers
//!   (npx resolving packages, python importing site-packages, dyld loading
//!   frameworks) so we invert the common case: read-mostly with a denylist.
//! - **Write**: denied everywhere except the per-run tempdir, `/private/var/tmp`,
//!   `/private/tmp`, and `~/Library/Caches/mcp-proxy/<server-id>/`.
//! - **Network**: `network-outbound` allowed by default (most MCP servers hit
//!   APIs). A `network: Blocked` opt-in clamps it off.
//! - **Exec**: `process-exec*` allowed — `npx` / `uvx` fan out to many child
//!   processes and gating each would be a compatibility nightmare.
//! - **stdio**: pipes are allowed, so inherited stdio works with no extra opt-in.
//! - **Mach lookups**: allowed broadly. Denying these breaks `dyld`, libSystem,
//!   and anything touching CoreFoundation.
//!
//! Users can always audit the generated profile — it's written to a real file
//! on disk before the exec.
//!
//! # Safety
//!
//! - `escape_scheme_string` quotes all dynamic path segments to prevent a
//!   malicious server name from injecting new sandbox rules. Unit-tested.
//! - If `sandbox-exec` is somehow absent we log a warning and fall back to a
//!   direct spawn. A malformed profile causes `sandbox-exec` to exit non-zero
//!   before running the child; its stderr is inherited so the user sees why.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::path::{Path, PathBuf};

/// Network posture for a sandboxed Local-mode child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxNetwork {
    /// Allow outbound connections. This is the default — most MCP servers need
    /// to talk to third-party APIs.
    Allowed,
    /// Deny all network access. Useful for filesystem-only servers.
    #[allow(dead_code)]
    Blocked,
}

impl Default for SandboxNetwork {
    fn default() -> Self {
        Self::Allowed
    }
}

/// Generate the body of a `.sb` sandbox profile.
///
/// `server_id` is used only to construct the per-server cache directory path
/// inside the profile; it is escaped before being embedded.
pub fn generate_profile(server_id: &str, cache_dir: &Path, network: SandboxNetwork) -> String {
    let cache_path = escape_scheme_string(&cache_dir.display().to_string());
    let sid = escape_scheme_string(server_id);
    // We don't actually use sid in the body right now (cache_dir already
    // encodes it), but include it as a leading comment so an auditor reading
    // the file can tell which server it belongs to.

    let network_rule = match network {
        SandboxNetwork::Allowed => "(allow network*)",
        SandboxNetwork::Blocked => "; network denied by default",
    };

    format!(
        r#";; mcp-proxy sandbox profile for server {sid}
;; Generated automatically — safe to audit or edit for a one-off run.

(version 1)
(deny default)
(debug deny)

;; ---- Process exec ----------------------------------------------------------
;; npx / uvx / node / python spawn many helpers. Gate none of them.
(allow process-exec*)
(allow process-fork)
(allow signal (target self))

;; ---- Filesystem: read-mostly with a denylist ------------------------------
;; Narrower allowlists break legitimate servers (dyld, site-packages, etc.),
;; so we invert: allow broad reads and explicitly deny secret stores.
(allow file-read*)
(deny file-read*
    (subpath (string-param "HOME" "/.ssh"))
    (subpath (string-param "HOME" "/.aws"))
    (subpath (string-param "HOME" "/.config/gh"))
    (subpath (string-param "HOME" "/.gnupg"))
    (subpath (string-param "HOME" "/Library/Keychains"))
    (literal "/private/etc/master.passwd")
    (literal "/etc/master.passwd")
    (literal "/private/etc/sudoers"))

;; ---- Filesystem: write is tightly scoped ----------------------------------
(allow file-write*
    (subpath "/private/tmp")
    (subpath "/private/var/tmp")
    (subpath "/private/var/folders")  ; $TMPDIR lives here on macOS
    (subpath {cache_path}))
;; stdin / stdout / stderr pipes — must stay writable so the MCP client can
;; talk to the server over inherited stdio.
(allow file-write-data (path "/dev/null"))
(allow file-write-data (path "/dev/dtracehelper"))
(allow file-ioctl (path "/dev/dtracehelper"))

;; ---- IPC + syscall essentials ---------------------------------------------
(allow mach-lookup)
(allow mach-register)
(allow ipc-posix-shm)
(allow sysctl-read)
(allow system-socket)
(allow iokit-open)

;; ---- Network --------------------------------------------------------------
{network_rule}
"#
    )
}

/// Escape a string for safe embedding inside a Scheme-style `.sb` profile.
///
/// Sandbox profiles are parsed by a TinyScheme derivative. Double-quoted
/// strings use `\"` for a literal double quote and `\\` for a literal
/// backslash. That's all we need: the rule grammar is statement-level, so as
/// long as the string never breaks out of its quotes it can't inject rules.
fn escape_scheme_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            // Control chars: drop them. Paths and server IDs never legitimately
            // contain NUL / DEL / etc., and TinyScheme's string reader chokes
            // on them.
            c if (c as u32) < 0x20 => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// On-disk sandbox profile that is unlinked when dropped.
pub struct TempProfile {
    path: PathBuf,
}

impl TempProfile {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempProfile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Write a freshly-generated profile into the per-run temp dir and return a
/// RAII handle that removes it on drop.
pub fn write_temp_profile(
    server_id: &str,
    cache_dir: &Path,
    network: SandboxNetwork,
) -> std::io::Result<TempProfile> {
    let mut base = std::env::temp_dir();
    // Random-ish suffix: PID + nanos is good enough for per-run uniqueness.
    let suffix = format!(
        "mcp-proxy-sandbox-{}-{}.sb",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    base.push(suffix);
    let profile = generate_profile(server_id, cache_dir, network);
    std::fs::write(&base, profile)?;
    Ok(TempProfile { path: base })
}

/// Returns the `~/Library/Caches/mcp-proxy/<server-id>/` path the sandbox is
/// allowed to write to, creating it if needed.
pub fn cache_dir_for(server_id: &str) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    let dir = home
        .join("Library")
        .join("Caches")
        .join("mcp-proxy")
        .join(sanitize_for_path(server_id));
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Strip path separators and odd control characters from a server id so it's
/// safe to use as a single directory-component.
fn sanitize_for_path(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '\0' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_quotes_regular_strings() {
        assert_eq!(escape_scheme_string("hello"), "\"hello\"");
    }

    #[test]
    fn escape_escapes_double_quote() {
        assert_eq!(escape_scheme_string(r#"he"llo"#), "\"he\\\"llo\"");
    }

    #[test]
    fn escape_escapes_backslash() {
        assert_eq!(escape_scheme_string(r"C:\foo"), "\"C:\\\\foo\"");
    }

    #[test]
    fn escape_drops_control_chars() {
        let s = "a\0b\x01c";
        assert_eq!(escape_scheme_string(s), "\"abc\"");
    }

    /// A malicious server id / path must not be able to escape its quoted
    /// string and inject a new sandbox rule like `(allow default)`.
    #[test]
    fn escape_prevents_rule_injection() {
        let hostile = r#"evil") (allow default) ("#;
        let escaped = escape_scheme_string(hostile);
        // The original `")` must be escaped out, so the closing-quote never
        // appears bare inside the embedded literal.
        assert!(!escaped.contains("\") ("));
        assert!(escaped.starts_with('"') && escaped.ends_with('"'));
    }

    #[test]
    fn generate_profile_includes_cache_path() {
        let profile = generate_profile(
            "srv-1",
            Path::new("/Users/me/Library/Caches/mcp-proxy/srv-1"),
            SandboxNetwork::Allowed,
        );
        assert!(profile.contains("(deny default)"));
        assert!(profile.contains("\"/Users/me/Library/Caches/mcp-proxy/srv-1\""));
        assert!(profile.contains("(allow network*)"));
    }

    #[test]
    fn generate_profile_blocks_network_when_requested() {
        let profile =
            generate_profile("srv", Path::new("/tmp/x"), SandboxNetwork::Blocked);
        assert!(!profile.contains("(allow network*)"));
        assert!(profile.contains("network denied by default"));
    }

    #[test]
    fn generate_profile_hostile_server_id_is_quoted() {
        let profile = generate_profile(
            r#"evil") (allow default) ("#,
            Path::new("/tmp/x"),
            SandboxNetwork::Allowed,
        );
        // The hostile string must appear only inside a quoted literal.
        assert!(!profile.contains("(allow default)"));
    }

    #[test]
    fn sanitize_for_path_replaces_separators() {
        assert_eq!(sanitize_for_path("a/b:c\\d"), "a_b_c_d");
    }

    #[test]
    fn write_temp_profile_then_drop_removes_file() {
        let tmp =
            write_temp_profile("test-server", Path::new("/tmp"), SandboxNetwork::Allowed)
                .expect("write profile");
        let path = tmp.path().to_path_buf();
        assert!(path.exists(), "profile should exist on disk");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("mcp-proxy sandbox profile"));
        drop(tmp);
        assert!(!path.exists(), "profile should be removed on drop");
    }
}
