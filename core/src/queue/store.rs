//! `QueueStore`: owns the SQLite connection and bridges blocking `rusqlite`
//! into async via `spawn_blocking`, delegating all SQL to `QueueRepository`.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;

use super::entry::{NewQueueEntry, QueueEntry, QueueStatus};
use super::repository::QueueRepository;
use super::schema;
use crate::stream::ResolvedFormat;

#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("queue database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("internal error: queue task panicked: {0}")]
    Join(#[from] tokio::task::JoinError),
}

/// Opens (and owns) the queue's SQLite database. Cheap to clone-and-share
/// via `Arc` if needed; internally the connection is already behind one.
pub struct QueueStore {
    conn: Arc<Mutex<Connection>>,
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

    pub(crate) fn from_connection(conn: Connection) -> Result<Self, QueueError> {
        schema::ensure_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Adds a new entry in `Queued` status and returns it as persisted.
    pub async fn add_entry(&self, entry: NewQueueEntry) -> Result<QueueEntry, QueueError> {
        self.with_repository(move |repo| repo.insert(entry)).await
    }

    /// Looks up a single entry by id, if it exists.
    pub async fn get_entry(&self, id: i64) -> Result<Option<QueueEntry>, QueueError> {
        self.with_repository(move |repo| repo.get(id)).await
    }

    /// Updates an entry's status (and clears/sets its error message).
    pub async fn set_status(
        &self,
        id: i64,
        status: QueueStatus,
        error_message: Option<&str>,
    ) -> Result<(), QueueError> {
        let error_message = error_message.map(str::to_string);
        self.with_repository(move |repo| repo.set_status(id, status, error_message))
            .await
    }

    /// Repoints an entry at a different stream format, resetting it to
    /// `Queued` with its error cleared. Returns the updated entry, or
    /// `None` if it no longer exists.
    pub async fn set_format(
        &self,
        id: i64,
        format: &ResolvedFormat,
    ) -> Result<Option<QueueEntry>, QueueError> {
        let (itag, quality_label, convert_to_mp3) = (
            format.itag,
            format.quality_label.clone(),
            format.convert_to_mp3,
        );
        self.with_repository(move |repo| repo.set_format(id, itag, quality_label, convert_to_mp3))
            .await
    }

    /// Deletes an entry. A no-op (not an error) if it doesn't exist.
    pub async fn delete_entry(&self, id: i64) -> Result<(), QueueError> {
        self.with_repository(move |repo| repo.delete(id)).await
    }

    /// Lists all entries, most recently added first.
    pub async fn list_entries(&self) -> Result<Vec<QueueEntry>, QueueError> {
        self.with_repository(|repo| repo.list()).await
    }

    /// Runs one repository operation on a blocking thread with the
    /// connection locked for its duration.
    async fn with_repository<T, F>(&self, op: F) -> Result<T, QueueError>
    where
        T: Send + 'static,
        F: FnOnce(QueueRepository<'_>) -> Result<T, rusqlite::Error> + Send + 'static,
    {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().expect("queue db mutex poisoned");
            op(QueueRepository::new(&conn)).map_err(QueueError::from)
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
            convert_to_mp3: false,
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
    async fn convert_to_mp3_flag_roundtrips() {
        let store = QueueStore::open_in_memory().unwrap();
        let added = store
            .add_entry(NewQueueEntry {
                convert_to_mp3: true,
                itag: 140,
                quality_label: None,
                ..new_entry("abc123")
            })
            .await
            .unwrap();
        assert!(added.convert_to_mp3);

        let fetched = store.get_entry(added.id).await.unwrap().unwrap();
        assert!(fetched.convert_to_mp3);
        assert!(store.list_entries().await.unwrap()[0].convert_to_mp3);
    }

    #[tokio::test]
    async fn opening_a_pre_mp3_database_adds_the_column() {
        // Simulate a database created before the convert_to_mp3 column
        // existed, then reopen it through the current schema logic.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE queue_entries (
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
            INSERT INTO queue_entries
                (video_id, title, itag, quality_label, output_path, status, error_message, created_at)
             VALUES ('old123', 'Old Entry', 18, '360p', '/tmp', 'queued', NULL, 1);",
        )
        .unwrap();

        let store = QueueStore::from_connection(conn).unwrap();
        let listed = store.list_entries().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].video_id, "old123");
        assert!(!listed[0].convert_to_mp3);
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
    async fn set_format_repoints_the_entry_and_requeues_it() {
        let store = QueueStore::open_in_memory().unwrap();
        let added = store.add_entry(new_entry("abc123")).await.unwrap();
        // A previously failed attempt: its error and status must not
        // survive being pointed at a different format.
        store
            .set_status(added.id, QueueStatus::Failed, Some("network error"))
            .await
            .unwrap();

        let updated = store
            .set_format(
                added.id,
                &ResolvedFormat {
                    itag: 140,
                    quality_label: None,
                    convert_to_mp3: true,
                },
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.itag, 140);
        assert_eq!(updated.quality_label, None);
        assert!(updated.convert_to_mp3);
        assert_eq!(updated.status, QueueStatus::Queued);
        assert_eq!(updated.error_message, None);
    }

    #[tokio::test]
    async fn set_format_reports_a_missing_entry_as_none() {
        let store = QueueStore::open_in_memory().unwrap();
        let missing = store
            .set_format(
                404,
                &ResolvedFormat {
                    itag: 140,
                    quality_label: None,
                    convert_to_mp3: true,
                },
            )
            .await
            .unwrap();
        assert!(missing.is_none());
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
