//! Bulk queue operations driven by a `FormatPreference` rather than an exact
//! itag, resolving each video's own format list individually. A video that
//! fails to resolve is skipped and reported rather than aborting the batch.

use crate::queue::{NewQueueEntry, QueueEntry, QueueError, QueueStore};
use crate::stream::{FormatPreference, StreamClient};

/// One video that couldn't be processed, and why.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnqueueSkip {
    pub video_id: String,
    pub reason: String,
}

/// The result of a bulk add: what landed in the queue, and what didn't.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnqueueOutcome {
    pub added: Vec<QueueEntry>,
    pub skipped: Vec<EnqueueSkip>,
}

/// The result of a bulk re-format: the updated entries, and what didn't
/// resolve.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReformatOutcome {
    pub updated: Vec<QueueEntry>,
    pub skipped: Vec<EnqueueSkip>,
}

/// Resolves each video's preferred format and adds it to the queue, one at
/// a time (the lists involved are typically small enough that this isn't
/// worth parallelizing yet — see `docs/ARCHITECTURE.md`).
///
/// `videos` entries may be full YouTube URLs or bare 11-character ids; the
/// resolution step accepts either. The only error returned is a queue-store
/// failure, since that affects every remaining video too — per-video
/// failures land in `skipped`.
pub async fn enqueue_videos(
    stream_client: &StreamClient,
    queue_store: &QueueStore,
    videos: &[String],
    preference: FormatPreference,
    output_path: &str,
) -> Result<EnqueueOutcome, QueueError> {
    let mut added = Vec::with_capacity(videos.len());
    let mut skipped = Vec::new();

    for video in videos {
        match stream_client.resolve_queue_format(video, preference).await {
            Ok((detail, format)) => {
                let entry = queue_store
                    .add_entry(NewQueueEntry {
                        video_id: detail.video_id,
                        title: detail.title,
                        itag: format.itag,
                        quality_label: format.quality_label,
                        output_path: output_path.to_string(),
                        convert_to_mp3: format.convert_to_mp3,
                    })
                    .await?;
                added.push(entry);
            }
            Err(e) => skipped.push(EnqueueSkip {
                video_id: video.clone(),
                reason: e.to_string(),
            }),
        }
    }

    Ok(EnqueueOutcome { added, skipped })
}

/// Re-resolves existing queue entries against a new preference and updates
/// their format in place, resetting each to `Queued` (see
/// [`QueueStore::set_format`]).
///
/// Entries that are missing, or currently downloading, are skipped rather
/// than errored — the same "one bad item doesn't sink the batch" rule as
/// [`enqueue_videos`].
pub async fn reformat_entries(
    stream_client: &StreamClient,
    queue_store: &QueueStore,
    queue_ids: &[i64],
    preference: FormatPreference,
) -> Result<ReformatOutcome, QueueError> {
    use crate::queue::QueueStatus;

    let mut updated = Vec::with_capacity(queue_ids.len());
    let mut skipped = Vec::new();

    for &queue_id in queue_ids {
        let Some(entry) = queue_store.get_entry(queue_id).await? else {
            skipped.push(EnqueueSkip {
                video_id: queue_id.to_string(),
                reason: "queue entry no longer exists".to_string(),
            });
            continue;
        };
        if entry.status == QueueStatus::Downloading {
            skipped.push(EnqueueSkip {
                video_id: entry.video_id,
                reason: "entry is downloading; cancel it before changing its format".to_string(),
            });
            continue;
        }

        match stream_client
            .resolve_queue_format(&entry.video_id, preference)
            .await
        {
            Ok((_, format)) => {
                if let Some(entry) = queue_store.set_format(queue_id, &format).await? {
                    updated.push(entry);
                }
            }
            Err(e) => skipped.push(EnqueueSkip {
                video_id: entry.video_id,
                reason: e.to_string(),
            }),
        }
    }

    Ok(ReformatOutcome { updated, skipped })
}
