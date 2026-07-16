//! Playlist import: bulk-add a playlist's videos to the download queue.
//!
//! `playlistItems.list` (`core::youtube`) only gives metadata — no format
//! info — so each video's own format list still has to be resolved
//! individually against a shared [`FormatPreference`] (`core::stream`),
//! exactly like the single-video "view formats" flow already does one at a
//! time. A video that fails to resolve (deleted, private, region-locked,
//! no format matching the preference) is skipped and reported rather than
//! aborting the whole import.

use crate::queue::{NewQueueEntry, QueueEntry, QueueError, QueueStore};
use crate::stream::{FormatPreference, StreamClient};

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaylistImportSkip {
    pub video_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlaylistImportOutcome {
    pub added: Vec<QueueEntry>,
    pub skipped: Vec<PlaylistImportSkip>,
}

/// Resolves each `video_id`'s preferred format and adds it to the queue,
/// one at a time (playlists are typically small enough that this isn't
/// worth parallelizing yet — see `docs/ARCHITECTURE.md`). A per-video
/// failure is recorded in `skipped` rather than stopping the import; the
/// only error this itself returns is a queue-store failure, since that
/// affects every remaining video too.
pub async fn import_videos_to_queue(
    stream_client: &StreamClient,
    queue_store: &QueueStore,
    video_ids: &[String],
    preference: FormatPreference,
    output_path: &str,
) -> Result<PlaylistImportOutcome, QueueError> {
    let mut added = Vec::with_capacity(video_ids.len());
    let mut skipped = Vec::new();

    for video_id in video_ids {
        match stream_client
            .resolve_preferred_format(video_id, preference)
            .await
        {
            Ok((detail, format)) => {
                let entry = queue_store
                    .add_entry(NewQueueEntry {
                        video_id: detail.video_id,
                        title: detail.title,
                        itag: format.itag,
                        quality_label: format.quality_label,
                        output_path: output_path.to_string(),
                        convert_to_mp3: false,
                    })
                    .await?;
                added.push(entry);
            }
            Err(e) => skipped.push(PlaylistImportSkip {
                video_id: video_id.clone(),
                reason: e.to_string(),
            }),
        }
    }

    Ok(PlaylistImportOutcome { added, skipped })
}
