//! Invocation tracking for MCP server runs.
//!
//! The CLI (`mcp-proxy run <id>`) opens a `sessions` row when a child MCP
//! server is launched and records every line of JSON-RPC traffic that flows
//! through its stdio tee into `tool_calls`. The desktop app reads these rows
//! to power the Activity page.
//!
//! Design notes:
//! - Backed by SQLite (`$data_dir/invocations.db`) for cheap aggregate
//!   queries (counts, tool histograms) — audit.log's JSONL is append-only
//!   and awkward for those.
//! - Writes hop through a bounded channel onto a background thread so the
//!   stdio hot path never blocks on disk.
//! - Logging failures are *best-effort*: we log a warning and keep the
//!   session running.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread::JoinHandle;

use crate::store::invocations_db_path;

const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Request,
    Response,
    Notification,
}

impl Direction {
    fn as_str(self) -> &'static str {
        match self {
            Direction::Request => "request",
            Direction::Response => "response",
            Direction::Notification => "notification",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRow {
    pub id: i64,
    pub server_id: String,
    pub run_mode: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i64>,
    pub error: Option<String>,
    pub tool_call_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRow {
    pub id: i64,
    pub session_id: i64,
    pub direction: String,
    pub method: Option<String>,
    pub tool_name: Option<String>,
    pub jsonrpc_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: Option<i64>,
    pub is_error: bool,
    pub payload: String,
}

pub fn open_db(path: &PathBuf) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode=WAL;
        PRAGMA foreign_keys=ON;
        CREATE TABLE IF NOT EXISTS sessions (
          id           INTEGER PRIMARY KEY AUTOINCREMENT,
          server_id    TEXT NOT NULL,
          run_mode     TEXT NOT NULL,
          started_at   TEXT NOT NULL,
          ended_at     TEXT,
          exit_code    INTEGER,
          error        TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_server
          ON sessions(server_id, started_at DESC);
        CREATE TABLE IF NOT EXISTS tool_calls (
          id            INTEGER PRIMARY KEY AUTOINCREMENT,
          session_id    INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
          direction     TEXT NOT NULL,
          method        TEXT,
          tool_name     TEXT,
          jsonrpc_id    TEXT,
          timestamp     TEXT NOT NULL,
          duration_ms   INTEGER,
          is_error      INTEGER NOT NULL DEFAULT 0,
          payload       TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_tool_calls_session
          ON tool_calls(session_id, timestamp);
        CREATE INDEX IF NOT EXISTS idx_tool_calls_jsonrpc_id
          ON tool_calls(session_id, jsonrpc_id);
        "#,
    )?;
    Ok(conn)
}

/// Events sent from the hot path to the logger thread.
enum LogEvent {
    Line {
        direction: Direction,
        line: String,
        timestamp: DateTime<Utc>,
    },
    Finish {
        exit_code: Option<i32>,
        error: Option<String>,
        timestamp: DateTime<Utc>,
    },
}

/// Lightweight cloneable handle threads can hold onto to record lines.
#[derive(Clone)]
pub struct LoggerHandle {
    tx: mpsc::SyncSender<LogEvent>,
}

impl LoggerHandle {
    pub fn record_line(&self, direction: Direction, line: &str) {
        let _ = self.tx.try_send(LogEvent::Line {
            direction,
            line: line.to_string(),
            timestamp: Utc::now(),
        });
    }
}

pub struct InvocationLogger {
    tx: Option<mpsc::SyncSender<LogEvent>>,
    handle: Option<JoinHandle<()>>,
}

impl InvocationLogger {
    /// Open the DB, insert a new session row, and spawn the bg writer.
    pub fn start(server_id: &str, run_mode: &str) -> Result<Self, String> {
        let path = invocations_db_path();
        let conn = open_db(&path).map_err(|e| format!("open invocations.db: {e}"))?;
        let started_at = Utc::now();
        conn.execute(
            "INSERT INTO sessions (server_id, run_mode, started_at) VALUES (?1, ?2, ?3)",
            params![server_id, run_mode, started_at.to_rfc3339()],
        )
        .map_err(|e| format!("insert session: {e}"))?;
        let session_id = conn.last_insert_rowid();

        let (tx, rx) = mpsc::sync_channel::<LogEvent>(256);
        let handle = std::thread::spawn(move || {
            writer_loop(conn, session_id, rx);
        });
        Ok(Self {
            tx: Some(tx),
            handle: Some(handle),
        })
    }

    /// Record a JSON-RPC line. Non-blocking best-effort.
    pub fn record_line(&self, direction: Direction, line: &str) {
        if let Some(tx) = &self.tx {
            let _ = tx.try_send(LogEvent::Line {
                direction,
                line: line.to_string(),
                timestamp: Utc::now(),
            });
        }
    }

    /// Return a cloneable handle suitable for passing into worker threads.
    pub fn handle(&self) -> Option<LoggerHandle> {
        self.tx.as_ref().map(|tx| LoggerHandle { tx: tx.clone() })
    }

    pub fn finish(mut self, exit_code: Option<i32>, error: Option<String>) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(LogEvent::Finish {
                exit_code,
                error,
                timestamp: Utc::now(),
            });
            drop(tx);
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for InvocationLogger {
    fn drop(&mut self) {
        // If finish() wasn't called, close down gracefully.
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(LogEvent::Finish {
                exit_code: None,
                error: None,
                timestamp: Utc::now(),
            });
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn writer_loop(mut conn: Connection, session_id: i64, rx: mpsc::Receiver<LogEvent>) {
    while let Ok(evt) = rx.recv() {
        match evt {
            LogEvent::Line {
                direction,
                line,
                timestamp,
            } => {
                if let Err(e) = write_line(&mut conn, session_id, direction, &line, timestamp) {
                    tracing::debug!("invocation_log write_line: {e}");
                }
            }
            LogEvent::Finish {
                exit_code,
                error,
                timestamp,
            } => {
                let _ = conn.execute(
                    "UPDATE sessions SET ended_at = ?1, exit_code = ?2, error = ?3 WHERE id = ?4",
                    params![
                        timestamp.to_rfc3339(),
                        exit_code.map(|c| c as i64),
                        error,
                        session_id,
                    ],
                );
                return;
            }
        }
    }
}

fn write_line(
    conn: &mut Connection,
    session_id: i64,
    direction: Direction,
    line: &str,
    timestamp: DateTime<Utc>,
) -> rusqlite::Result<()> {
    let parsed: Option<serde_json::Value> = serde_json::from_str(line.trim()).ok();
    let (method, tool_name, jsonrpc_id, is_error) = match parsed.as_ref() {
        Some(v) => {
            let method = v.get("method").and_then(|m| m.as_str()).map(String::from);
            let tool_name = match method.as_deref() {
                Some("tools/call") => v
                    .get("params")
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .map(String::from),
                _ => None,
            };
            let jsonrpc_id = v.get("id").map(|id| match id {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            });
            let is_error = v.get("error").is_some();
            (method, tool_name, jsonrpc_id, is_error)
        }
        None => (None, None, None, false),
    };

    let payload = if line.len() > MAX_PAYLOAD_BYTES {
        let mut truncated = line[..MAX_PAYLOAD_BYTES].to_string();
        truncated.push_str("...[truncated]");
        truncated
    } else {
        line.to_string()
    };

    // Duration pairing: if this is a response with an id, look up matching
    // prior request and compute delta ms.
    let duration_ms: Option<i64> = if direction == Direction::Response {
        jsonrpc_id.as_ref().and_then(|id| {
            conn.query_row(
                "SELECT timestamp FROM tool_calls
                   WHERE session_id = ?1 AND jsonrpc_id = ?2 AND direction = 'request'
                   ORDER BY id DESC LIMIT 1",
                params![session_id, id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()
            .and_then(|ts| DateTime::parse_from_rfc3339(&ts).ok())
            .map(|req_ts| {
                (timestamp.timestamp_millis() - req_ts.with_timezone(&Utc).timestamp_millis())
                    .max(0)
            })
        })
    } else {
        None
    };

    conn.execute(
        "INSERT INTO tool_calls
           (session_id, direction, method, tool_name, jsonrpc_id,
            timestamp, duration_ms, is_error, payload)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            session_id,
            direction.as_str(),
            method,
            tool_name,
            jsonrpc_id,
            timestamp.to_rfc3339(),
            duration_ms,
            is_error as i64,
            payload,
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// read queries (Tauri-facing)
// ---------------------------------------------------------------------------

pub fn list_sessions(server_id: Option<&str>, limit: usize) -> Result<Vec<SessionRow>, String> {
    let path = invocations_db_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let conn = open_db(&path).map_err(|e| e.to_string())?;
    let sql = if server_id.is_some() {
        "SELECT s.id, s.server_id, s.run_mode, s.started_at, s.ended_at,
                s.exit_code, s.error,
                (SELECT COUNT(*) FROM tool_calls WHERE session_id = s.id) AS tc
           FROM sessions s
          WHERE s.server_id = ?1
          ORDER BY s.id DESC
          LIMIT ?2"
    } else {
        "SELECT s.id, s.server_id, s.run_mode, s.started_at, s.ended_at,
                s.exit_code, s.error,
                (SELECT COUNT(*) FROM tool_calls WHERE session_id = s.id) AS tc
           FROM sessions s
          ORDER BY s.id DESC
          LIMIT ?1"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = if let Some(sid) = server_id {
        stmt.query_map(params![sid, limit as i64], map_session_row)
    } else {
        stmt.query_map(params![limit as i64], map_session_row)
    }
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn map_session_row(row: &rusqlite::Row) -> rusqlite::Result<SessionRow> {
    let started: String = row.get(3)?;
    let ended: Option<String> = row.get(4)?;
    Ok(SessionRow {
        id: row.get(0)?,
        server_id: row.get(1)?,
        run_mode: row.get(2)?,
        started_at: DateTime::parse_from_rfc3339(&started)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        ended_at: ended
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc)),
        exit_code: row.get(5)?,
        error: row.get(6)?,
        tool_call_count: row.get(7)?,
    })
}

pub fn list_tool_calls(session_id: i64, limit: usize) -> Result<Vec<ToolCallRow>, String> {
    let path = invocations_db_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let conn = open_db(&path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, session_id, direction, method, tool_name, jsonrpc_id,
                    timestamp, duration_ms, is_error, payload
               FROM tool_calls
              WHERE session_id = ?1
              ORDER BY id ASC
              LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![session_id, limit as i64], |row| {
            let ts: String = row.get(6)?;
            Ok(ToolCallRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                direction: row.get(2)?,
                method: row.get(3)?,
                tool_name: row.get(4)?,
                jsonrpc_id: row.get(5)?,
                timestamp: DateTime::parse_from_rfc3339(&ts)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                duration_ms: row.get(7)?,
                is_error: row.get::<_, i64>(8)? != 0,
                payload: row.get(9)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn tool_call_counts(
    server_id: &str,
    since: DateTime<Utc>,
) -> Result<Vec<(String, i64)>, String> {
    let path = invocations_db_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let conn = open_db(&path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT tc.tool_name, COUNT(*)
               FROM tool_calls tc
               JOIN sessions s ON s.id = tc.session_id
              WHERE s.server_id = ?1
                AND tc.tool_name IS NOT NULL
                AND tc.direction = 'request'
                AND tc.timestamp >= ?2
              GROUP BY tc.tool_name
              ORDER BY 2 DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![server_id, since.to_rfc3339()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn prune_older_than_days(days: i64) -> Result<usize, String> {
    let path = invocations_db_path();
    if !path.exists() {
        return Ok(0);
    }
    let conn = open_db(&path).map_err(|e| e.to_string())?;
    let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    conn.execute(
        "DELETE FROM sessions WHERE started_at < ?1",
        params![cutoff],
    )
    .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{test_env_lock, DATA_DIR_ENV};
    use tempfile::TempDir;

    fn with_tempdir<F: FnOnce()>(f: F) {
        let _g = test_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        std::env::set_var(DATA_DIR_ENV, tmp.path());
        f();
        std::env::remove_var(DATA_DIR_ENV);
    }

    #[test]
    fn session_lifecycle_inserts_and_lists() {
        with_tempdir(|| {
            let logger = InvocationLogger::start("srv-1", "local").unwrap();
            logger.record_line(
                Direction::Request,
                r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"hello"}}"#,
            );
            logger.record_line(
                Direction::Response,
                r#"{"jsonrpc":"2.0","id":1,"result":{}}"#,
            );
            logger.record_line(
                Direction::Notification,
                r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#,
            );
            logger.finish(Some(0), None);

            let sessions = list_sessions(Some("srv-1"), 10).unwrap();
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].server_id, "srv-1");
            assert_eq!(sessions[0].tool_call_count, 3);
            assert_eq!(sessions[0].exit_code, Some(0));

            let calls = list_tool_calls(sessions[0].id, 100).unwrap();
            assert_eq!(calls.len(), 3);
            // Request row
            assert_eq!(calls[0].method.as_deref(), Some("tools/call"));
            assert_eq!(calls[0].tool_name.as_deref(), Some("hello"));
            // Response has duration_ms populated via id pairing
            assert!(calls[1].duration_ms.is_some());

            let hist = tool_call_counts("srv-1", Utc::now() - chrono::Duration::hours(1)).unwrap();
            assert_eq!(hist, vec![("hello".to_string(), 1)]);
        });
    }

    #[test]
    fn malformed_json_line_persisted_raw() {
        with_tempdir(|| {
            let logger = InvocationLogger::start("srv-2", "local").unwrap();
            logger.record_line(Direction::Request, "not json at all");
            logger.finish(Some(0), None);
            let sessions = list_sessions(Some("srv-2"), 10).unwrap();
            let calls = list_tool_calls(sessions[0].id, 10).unwrap();
            assert_eq!(calls.len(), 1);
            assert!(calls[0].method.is_none());
            assert_eq!(calls[0].payload, "not json at all");
        });
    }

    #[test]
    fn payload_truncated_when_oversized() {
        with_tempdir(|| {
            let logger = InvocationLogger::start("srv-3", "local").unwrap();
            let big = "x".repeat(MAX_PAYLOAD_BYTES + 100);
            logger.record_line(Direction::Request, &big);
            logger.finish(Some(0), None);
            let sessions = list_sessions(Some("srv-3"), 10).unwrap();
            let calls = list_tool_calls(sessions[0].id, 10).unwrap();
            assert!(calls[0].payload.ends_with("...[truncated]"));
            assert!(calls[0].payload.len() <= MAX_PAYLOAD_BYTES + "...[truncated]".len());
        });
    }
}
