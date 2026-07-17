//! Schema setup and migrations for the shared queue database (both the
//! `queue_entries` table and `core::agent`'s `pending_agent_actions` table,
//! which lives in the same file).

use rusqlite::Connection;

/// Creates missing tables and applies column migrations. Idempotent; runs
/// on every open.
pub(crate) fn ensure_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS queue_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            video_id TEXT NOT NULL,
            title TEXT NOT NULL,
            itag INTEGER NOT NULL,
            quality_label TEXT,
            output_path TEXT NOT NULL,
            convert_to_mp3 INTEGER NOT NULL DEFAULT 0,
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
    // Databases created before the MP3-conversion feature lack the column;
    // `CREATE TABLE IF NOT EXISTS` won't add it to an existing table, so
    // inspect and ALTER.
    let has_convert_column = conn
        .prepare("PRAGMA table_info(queue_entries)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|name| name == "convert_to_mp3");
    if !has_convert_column {
        conn.execute(
            "ALTER TABLE queue_entries ADD COLUMN convert_to_mp3 INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    Ok(())
}
