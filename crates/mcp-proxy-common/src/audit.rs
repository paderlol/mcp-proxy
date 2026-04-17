use crate::store::audit_log_path;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub timestamp: DateTime<Utc>,
    pub server_id: String,
    pub secret_id: String,
    pub source: String,
    pub status: AuditStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "message")]
pub enum AuditStatus {
    Success,
    Error(String),
}

pub fn append_audit_log(entry: &AuditLogEntry) -> Result<(), String> {
    let path = audit_log_path();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("Failed to open audit log {}: {e}", path.display()))?;

    let line = serde_json::to_string(entry)
        .map_err(|e| format!("Failed to serialize audit log entry: {e}"))?;
    writeln!(file, "{line}")
        .map_err(|e| format!("Failed to append audit log {}: {e}", path.display()))
}

pub fn read_recent_audit_logs(limit: usize) -> Result<Vec<AuditLogEntry>, String> {
    let path = audit_log_path();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(&path)
        .map_err(|e| format!("Failed to read audit log {}: {e}", path.display()))?;
    let reader = BufReader::new(file);

    let mut entries = reader
        .lines()
        .filter_map(|line| {
            let line = line.ok()?;
            serde_json::from_str::<AuditLogEntry>(&line).ok()
        })
        .collect::<Vec<_>>();

    if entries.len() > limit {
        entries = entries.split_off(entries.len() - limit);
    }
    entries.reverse();
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::DATA_DIR_ENV;
    use tempfile::TempDir;

    #[test]
    fn append_and_read_recent_logs_round_trip() {
        let tmp = TempDir::new().unwrap();
        unsafe { std::env::set_var(DATA_DIR_ENV, tmp.path()) };

        append_audit_log(&AuditLogEntry {
            timestamp: Utc::now(),
            server_id: "github".into(),
            secret_id: "github-pat".into(),
            source: "Local".into(),
            status: AuditStatus::Success,
        })
        .unwrap();
        append_audit_log(&AuditLogEntry {
            timestamp: Utc::now(),
            server_id: "brave".into(),
            secret_id: "brave-key".into(),
            source: "OnePassword".into(),
            status: AuditStatus::Error("missing".into()),
        })
        .unwrap();

        let entries = read_recent_audit_logs(10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].server_id, "brave");
        assert_eq!(entries[1].server_id, "github");
    }
}
