//! The queue's domain types: entry, status, and the new-entry payload.

use serde::{Deserialize, Serialize};

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
    /// The value persisted in the `status` column.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            QueueStatus::Queued => "queued",
            QueueStatus::Downloading => "downloading",
            QueueStatus::Completed => "completed",
            QueueStatus::Failed => "failed",
            QueueStatus::Cancelled => "cancelled",
        }
    }

    pub(crate) fn from_str(s: &str) -> Self {
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
    /// Transcode the downloaded audio stream to MP3 (and delete the
    /// original) once the download finishes — see `core::download`.
    pub convert_to_mp3: bool,
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
    /// `serde(default)` keeps pending agent-action payloads persisted
    /// before this field existed deserializable (they never convert).
    #[serde(default)]
    pub convert_to_mp3: bool,
}
