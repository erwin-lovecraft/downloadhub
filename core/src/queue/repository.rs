//! All SQL touching the `queue_entries` table, and nothing else.
//!
//! [`QueueRepository`] borrows an already-locked connection and is
//! constructed fresh for each operation by [`QueueStore`](super::QueueStore)
//! — it holds no state of its own, so creating and dropping one per call is
//! free. Locking, `spawn_blocking`, and error wrapping are the store's job;
//! this type only knows the table.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, Row};

use super::entry::{NewQueueEntry, QueueEntry, QueueStatus};

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

const SELECT_COLUMNS: &str = "id, video_id, title, itag, quality_label, output_path, \
                              convert_to_mp3, status, error_message, created_at";

pub(crate) struct QueueRepository<'c> {
    conn: &'c Connection,
}

impl<'c> QueueRepository<'c> {
    pub(crate) fn new(conn: &'c Connection) -> Self {
        Self { conn }
    }

    /// Inserts `entry` in `Queued` status and returns it as persisted.
    pub(crate) fn insert(&self, entry: NewQueueEntry) -> Result<QueueEntry, rusqlite::Error> {
        let created_at = now_unix();
        self.conn.execute(
            "INSERT INTO queue_entries
                (video_id, title, itag, quality_label, output_path, convert_to_mp3, status, error_message, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)",
            rusqlite::params![
                entry.video_id,
                entry.title,
                entry.itag,
                entry.quality_label,
                entry.output_path,
                entry.convert_to_mp3,
                QueueStatus::Queued.as_str(),
                created_at,
            ],
        )?;
        Ok(QueueEntry {
            id: self.conn.last_insert_rowid(),
            video_id: entry.video_id,
            title: entry.title,
            itag: entry.itag,
            quality_label: entry.quality_label,
            output_path: entry.output_path,
            convert_to_mp3: entry.convert_to_mp3,
            status: QueueStatus::Queued,
            error_message: None,
            created_at,
        })
    }

    pub(crate) fn get(&self, id: i64) -> Result<Option<QueueEntry>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM queue_entries WHERE id = ?1"
        ))?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_entry(row)?)),
            None => Ok(None),
        }
    }

    /// All entries, most recently added first.
    pub(crate) fn list(&self) -> Result<Vec<QueueEntry>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLUMNS} FROM queue_entries ORDER BY created_at DESC, id DESC"
        ))?;
        let rows = stmt.query_map([], row_to_entry)?;
        rows.collect()
    }

    /// Repoints an entry at a different stream format and resets it to
    /// `Queued` with its error cleared: the previous format's failure (or
    /// cancellation) says nothing about the new one, and a `Completed`
    /// entry re-formatted is a request to fetch it again. Returns `None`
    /// if the entry doesn't exist.
    pub(crate) fn set_format(
        &self,
        id: i64,
        itag: u32,
        quality_label: Option<String>,
        convert_to_mp3: bool,
    ) -> Result<Option<QueueEntry>, rusqlite::Error> {
        self.conn.execute(
            "UPDATE queue_entries
                SET itag = ?1, quality_label = ?2, convert_to_mp3 = ?3,
                    status = ?4, error_message = NULL
             WHERE id = ?5",
            rusqlite::params![
                itag,
                quality_label,
                convert_to_mp3,
                QueueStatus::Queued.as_str(),
                id,
            ],
        )?;
        self.get(id)
    }

    pub(crate) fn set_status(
        &self,
        id: i64,
        status: QueueStatus,
        error_message: Option<String>,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE queue_entries SET status = ?1, error_message = ?2 WHERE id = ?3",
            rusqlite::params![status.as_str(), error_message, id],
        )?;
        Ok(())
    }

    /// A no-op (not an error) if the entry doesn't exist.
    pub(crate) fn delete(&self, id: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM queue_entries WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }
}

fn row_to_entry(row: &Row<'_>) -> Result<QueueEntry, rusqlite::Error> {
    Ok(QueueEntry {
        id: row.get(0)?,
        video_id: row.get(1)?,
        title: row.get(2)?,
        itag: row.get(3)?,
        quality_label: row.get(4)?,
        output_path: row.get(5)?,
        convert_to_mp3: row.get(6)?,
        status: QueueStatus::from_str(&row.get::<_, String>(7)?),
        error_message: row.get(8)?,
        created_at: row.get(9)?,
    })
}
