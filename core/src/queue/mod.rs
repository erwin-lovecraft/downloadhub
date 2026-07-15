//! Download queue: SQLite-backed persistence for queued downloads.
//!
//! Uses `rusqlite` (with the `bundled` feature, which vendors SQLite's C
//! source) rather than `sqlx`: queue operations are simple single-row CRUD
//! with no need for an async-native driver or compile-time query checking,
//! and bundling avoids depending on a system SQLite install being present
//! on the target machine. `rusqlite::Connection` is blocking, so every
//! operation runs inside `tokio::task::spawn_blocking` to avoid stalling
//! the async runtime.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("queue database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("internal error: queue task panicked: {0}")]
    Join(#[from] tokio::task::JoinError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    Queued,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

impl QueueStatus {
    fn as_str(self) -> &'static str {
        match self {
            QueueStatus::Queued => "queued",
            QueueStatus::Downloading => "downloading",
            QueueStatus::Completed => "completed",
            QueueStatus::Failed => "failed",
            QueueStatus::Cancelled => "cancelled",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "downloading" => QueueStatus::Downloading,
            "completed" => QueueStatus::Completed,
            "failed" => QueueStatus::Failed,
            "cancelled" => QueueStatus::Cancelled,
            _ => QueueStatus::Queued,
        }
    }
}

/// A persisted queue entry.
#[derive(Debug, Clone, Serialize)]
pub struct QueueEntry {
    pub id: i64,
    pub video_id: String,
    pub title: String,
    pub itag: u32,
    pub quality_label: Option<String>,
    pub output_path: String,
    pub status: QueueStatus,
    pub error_message: Option<String>,
    /// Unix timestamp (seconds) the entry was added.
    pub created_at: i64,
}

/// Fields needed to add a new entry; `status`/`id`/`created_at` are assigned
/// by the store. Serde derives exist because `core::agent` embeds this in a
/// pending agent action's persisted JSON payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewQueueEntry {
    pub video_id: String,
    pub title: String,
    pub itag: u32,
    pub quality_label: Option<String>,
    pub output_path: String,
}

/// Opens (and owns) the queue's SQLite database. Cheap to clone-and-share
/// via `Arc` if needed; internally the connection is already behind one.
pub struct QueueStore {
    /// `pub(crate)` so `core::agent` can add its own methods on this store
    /// (its table lives in the same database file).
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl QueueStore {
    /// Opens (creating if missing) the database at `db_path` and ensures
    /// the schema exists. `db_path`'s parent directory must already exist.
    ///
    /// The database is opened in WAL mode with a busy timeout: the desktop
    /// app and the `mcp-server` binary are separate processes sharing this
    /// same file (see `docs/ARCHITECTURE.md`), and the default rollback
    /// journal would make one process's write error with `SQLITE_BUSY` the
    /// instant the other held any lock.
    pub fn open(db_path: &Path) -> Result<Self, QueueError> {
        let conn = Connection::open(db_path)?;
        // `PRAGMA journal_mode` returns the resulting mode as a row, so it
        // can't go through `execute_batch`.
        conn.query_row("PRAGMA journal_mode=WAL", [], |_row| Ok(()))?;
        conn.busy_timeout(Duration::from_secs(5))?;
        Self::from_connection(conn)
    }

    #[cfg(test)]
    pub(crate) fn open_in_memory() -> Result<Self, QueueError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self, QueueError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS queue_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                video_id TEXT NOT NULL,
                title TEXT NOT NULL,
                itag INTEGER NOT NULL,
                quality_label TEXT,
                output_path TEXT NOT NULL,
                status TEXT NOT NULL,
                error_message TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS pending_agent_actions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                payload TEXT NOT NULL,
                status TEXT NOT NULL,
                requested_by TEXT,
                error_message TEXT,
                created_at INTEGER NOT NULL,
                resolved_at INTEGER
            );",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Adds a new entry in `Queued` status and returns it as persisted.
    pub async fn add_entry(&self, entry: NewQueueEntry) -> Result<QueueEntry, QueueError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().expect("queue db mutex poisoned");
            let created_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            conn.execute(
                "INSERT INTO queue_entries
                    (video_id, title, itag, quality_label, output_path, status, error_message, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)",
                rusqlite::params![
                    entry.video_id,
                    entry.title,
                    entry.itag,
                    entry.quality_label,
                    entry.output_path,
                    QueueStatus::Queued.as_str(),
                    created_at,
                ],
            )?;
            Ok(QueueEntry {
                id: conn.last_insert_rowid(),
                video_id: entry.video_id,
                title: entry.title,
                itag: entry.itag,
                quality_label: entry.quality_label,
                output_path: entry.output_path,
                status: QueueStatus::Queued,
                error_message: None,
                created_at,
            })
        })
        .await?
    }

    /// Looks up a single entry by id, if it exists.
    pub async fn get_entry(&self, id: i64) -> Result<Option<QueueEntry>, QueueError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().expect("queue db mutex poisoned");
            let mut stmt = conn.prepare(
                "SELECT id, video_id, title, itag, quality_label, output_path, status, error_message, created_at
                 FROM queue_entries WHERE id = ?1",
            )?;
            let mut rows = stmt.query(rusqlite::params![id])?;
            match rows.next()? {
                Some(row) => Ok(Some(QueueEntry {
                    id: row.get(0)?,
                    video_id: row.get(1)?,
                    title: row.get(2)?,
                    itag: row.get(3)?,
                    quality_label: row.get(4)?,
                    output_path: row.get(5)?,
                    status: QueueStatus::from_str(&row.get::<_, String>(6)?),
                    error_message: row.get(7)?,
                    created_at: row.get(8)?,
                })),
                None => Ok(None),
            }
        })
        .await?
    }

    /// Updates an entry's status (and clears/sets its error message).
    pub async fn set_status(
        &self,
        id: i64,
        status: QueueStatus,
        error_message: Option<&str>,
    ) -> Result<(), QueueError> {
        let conn = self.conn.clone();
        let status = status.as_str();
        let error_message = error_message.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().expect("queue db mutex poisoned");
            conn.execute(
                "UPDATE queue_entries SET status = ?1, error_message = ?2 WHERE id = ?3",
                rusqlite::params![status, error_message, id],
            )?;
            Ok(())
        })
        .await?
    }

    /// Deletes an entry. A no-op (not an error) if it doesn't exist.
    pub async fn delete_entry(&self, id: i64) -> Result<(), QueueError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().expect("queue db mutex poisoned");
            conn.execute(
                "DELETE FROM queue_entries WHERE id = ?1",
                rusqlite::params![id],
            )?;
            Ok(())
        })
        .await?
    }

    /// Lists all entries, most recently added first.
    pub async fn list_entries(&self) -> Result<Vec<QueueEntry>, QueueError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().expect("queue db mutex poisoned");
            let mut stmt = conn.prepare(
                "SELECT id, video_id, title, itag, quality_label, output_path, status, error_message, created_at
                 FROM queue_entries ORDER BY created_at DESC, id DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(QueueEntry {
                    id: row.get(0)?,
                    video_id: row.get(1)?,
                    title: row.get(2)?,
                    itag: row.get(3)?,
                    quality_label: row.get(4)?,
                    output_path: row.get(5)?,
                    status: QueueStatus::from_str(&row.get::<_, String>(6)?),
                    error_message: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>().map_err(QueueError::from)
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_entry(video_id: &str) -> NewQueueEntry {
        NewQueueEntry {
            video_id: video_id.to_string(),
            title: "Test Video".to_string(),
            itag: 18,
            quality_label: Some("360p".to_string()),
            output_path: "C:/downloads/test.mp4".to_string(),
        }
    }

    #[tokio::test]
    async fn add_then_list_returns_the_entry() {
        let store = QueueStore::open_in_memory().unwrap();
        let added = store.add_entry(new_entry("abc123")).await.unwrap();
        assert_eq!(added.status, QueueStatus::Queued);
        assert!(added.id > 0);

        let listed = store.list_entries().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].video_id, "abc123");
        assert_eq!(listed[0].itag, 18);
        assert_eq!(listed[0].quality_label.as_deref(), Some("360p"));
    }

    #[tokio::test]
    async fn list_orders_most_recent_first() {
        let store = QueueStore::open_in_memory().unwrap();
        store.add_entry(new_entry("first")).await.unwrap();
        store.add_entry(new_entry("second")).await.unwrap();

        let listed = store.list_entries().await.unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].video_id, "second");
        assert_eq!(listed[1].video_id, "first");
    }

    #[tokio::test]
    async fn get_entry_finds_by_id_and_none_when_missing() {
        let store = QueueStore::open_in_memory().unwrap();
        let added = store.add_entry(new_entry("abc123")).await.unwrap();

        let found = store.get_entry(added.id).await.unwrap();
        assert_eq!(found.unwrap().video_id, "abc123");

        let missing = store.get_entry(added.id + 1).await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn set_status_updates_status_and_error_message() {
        let store = QueueStore::open_in_memory().unwrap();
        let added = store.add_entry(new_entry("abc123")).await.unwrap();

        store
            .set_status(added.id, QueueStatus::Failed, Some("network error"))
            .await
            .unwrap();

        let updated = store.get_entry(added.id).await.unwrap().unwrap();
        assert_eq!(updated.status, QueueStatus::Failed);
        assert_eq!(updated.error_message.as_deref(), Some("network error"));
    }

    #[tokio::test]
    async fn delete_entry_removes_it_and_is_a_noop_when_missing() {
        let store = QueueStore::open_in_memory().unwrap();
        let added = store.add_entry(new_entry("abc123")).await.unwrap();

        store.delete_entry(added.id).await.unwrap();
        assert!(store.get_entry(added.id).await.unwrap().is_none());
        assert_eq!(store.list_entries().await.unwrap().len(), 0);

        // Deleting again (or an id that never existed) is not an error.
        store.delete_entry(added.id).await.unwrap();
    }
}
