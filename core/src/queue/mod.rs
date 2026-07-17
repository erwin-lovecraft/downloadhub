//! Download queue: SQLite-backed persistence for queued downloads.
//!
//! Uses `rusqlite` (with the `bundled` feature, which vendors SQLite's C
//! source) rather than `sqlx`: queue operations are simple single-row CRUD
//! with no need for an async-native driver or compile-time query checking,
//! and bundling avoids depending on a system SQLite install being present
//! on the target machine.
//!
//! Layout, one responsibility per file:
//!
//! - [`entry`]: the domain types (`QueueEntry`, `QueueStatus`, ...)
//! - [`schema`]: table creation + migrations
//! - [`repository`]: `QueueRepository`, the only place SQL against
//!   `queue_entries` lives (synchronous, on a borrowed connection)
//! - [`store`]: `QueueStore`, connection ownership + the async facade

mod entry;
mod repository;
mod schema;
mod store;

pub use entry::{NewQueueEntry, QueueEntry, QueueStatus};
pub use store::{QueueError, QueueStore};

pub(crate) use repository::now_unix;
