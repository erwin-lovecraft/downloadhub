//! Download queue: SQLite-backed persistence for queued downloads, via
//! `rusqlite` with the `bundled` feature (no system SQLite needed).

mod entry;
mod repository;
mod schema;
mod store;

pub use entry::{NewQueueEntry, QueueEntry, QueueStatus};
pub use store::{QueueError, QueueStore};
