//! Bulk queue operations driven by a `FormatPreference` rather than an exact
//! itag. A video that can't be processed is skipped and reported rather than
//! aborting the batch.

use std::collections::HashMap;

use crate::queue::{NewQueueEntry, QueueEntry, QueueError, QueueStore};
use crate::stream::{FormatPreference, StreamClient, YtDlpConfig};
use crate::youtube::{extract_video_id, YoutubeClient};

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

/// Adds each video to the queue **without** looking up its format list.
///
/// The itag recorded is `itag_override`, or the one
/// [`FormatPreference::presumed_itag`] assumes for `preference`. Nothing here
/// verifies the video actually offers it, and that is the point: resolving a
/// real itag costs one yt-dlp process per video (seconds each, sequentially),
/// while the download path fetches the format list again anyway and can
/// recover — an MP3 entry substitutes any audio stream on its own, and any
/// other entry fails with the itag named, which the user fixes by picking a
/// real format in the app (`set_queue_entry_format`).
///
/// `videos` entries may be full YouTube URLs or bare 11-character ids;
/// anything else is skipped and reported. Titles come from `youtube` in one
/// batched `videos.list` call rather than per video. Without a client, or if
/// that call fails, entries are queued titled by their video id and the
/// download path corrects them once it has the real metadata — queueing is
/// the caller's actual goal, and a title lookup shouldn't be able to sink it.
///
/// The only error returned is a queue-store failure, since that affects every
/// remaining video too.
pub async fn enqueue_videos(
    youtube: Option<&YoutubeClient>,
    queue_store: &QueueStore,
    videos: &[String],
    preference: FormatPreference,
    output_path: &str,
    itag_override: Option<u32>,
) -> Result<EnqueueOutcome, QueueError> {
    let mut added = Vec::with_capacity(videos.len());
    let mut skipped = Vec::new();
    let mut video_ids = Vec::with_capacity(videos.len());

    for video in videos {
        match extract_video_id(video) {
            Some(id) => video_ids.push(id),
            None => skipped.push(EnqueueSkip {
                video_id: video.clone(),
                reason: "not a YouTube video URL or 11-character video id".to_string(),
            }),
        }
    }

    let titles = fetch_titles(youtube, &video_ids).await;
    let itag = itag_override.unwrap_or_else(|| preference.presumed_itag());

    for video_id in video_ids {
        let title = titles
            .get(&video_id)
            .cloned()
            .unwrap_or_else(|| video_id.clone());
        let entry = queue_store
            .add_entry(NewQueueEntry {
                video_id,
                title,
                itag,
                quality_label: preference.presumed_quality_label(),
                output_path: output_path.to_string(),
                convert_to_mp3: preference.convert_to_mp3(),
            })
            .await?;
        added.push(entry);
    }

    Ok(EnqueueOutcome { added, skipped })
}

/// Titles keyed by video id, best-effort: an absent client or a failed API
/// call yields an empty map rather than an error (see [`enqueue_videos`]).
async fn fetch_titles(
    youtube: Option<&YoutubeClient>,
    video_ids: &[String],
) -> HashMap<String, String> {
    let Some(youtube) = youtube else {
        return HashMap::new();
    };
    youtube.fetch_titles(video_ids).await.unwrap_or_else(|e| {
        eprintln!("enqueue: title lookup failed, queueing by video id instead: {e}");
        HashMap::new()
    })
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
    ytdlp_config: &YtDlpConfig,
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
            .resolve_queue_format(&entry.video_id, preference, ytdlp_config)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{AUTO_AUDIO_ITAG, MP3_SOURCE_ITAG, PROGRESSIVE_ITAG};

    async fn store() -> QueueStore {
        QueueStore::open_in_memory().expect("in-memory queue db")
    }

    /// The whole point of the rewrite: no `StreamClient` is passed, and no
    /// yt-dlp process runs, so this test needs no provider at all.
    #[tokio::test]
    async fn queues_without_looking_up_any_format() {
        let store = store().await;
        let videos = vec![
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
            "aaaaaaaaaaa".to_string(),
        ];

        let outcome = enqueue_videos(
            None,
            &store,
            &videos,
            FormatPreference::Mp3,
            "/tmp/out",
            None,
        )
        .await
        .unwrap();

        assert_eq!(outcome.skipped.len(), 0);
        assert_eq!(outcome.added.len(), 2);
        assert_eq!(outcome.added[0].video_id, "dQw4w9WgXcQ");
        assert_eq!(outcome.added[1].video_id, "aaaaaaaaaaa");
        assert_eq!(store.list_entries().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn records_the_itag_each_preference_presumes() {
        let store = store().await;
        let videos = vec!["dQw4w9WgXcQ".to_string()];

        for (preference, expected) in [
            (FormatPreference::Mp3, AUTO_AUDIO_ITAG),
            (FormatPreference::BestAudioOnly, MP3_SOURCE_ITAG),
            (FormatPreference::BestProgressive, PROGRESSIVE_ITAG),
        ] {
            let outcome = enqueue_videos(None, &store, &videos, preference, "/tmp/out", None)
                .await
                .unwrap();
            assert_eq!(outcome.added[0].itag, expected, "for {preference:?}");
            assert_eq!(
                outcome.added[0].convert_to_mp3,
                preference == FormatPreference::Mp3
            );
        }
    }

    #[tokio::test]
    async fn an_itag_override_wins_over_the_presumed_one() {
        let store = store().await;
        let outcome = enqueue_videos(
            None,
            &store,
            &["dQw4w9WgXcQ".to_string()],
            FormatPreference::Mp3,
            "/tmp/out",
            Some(251),
        )
        .await
        .unwrap();

        assert_eq!(outcome.added[0].itag, 251);
        // The override changes the stream, never whether it's transcoded:
        // that stays a property of the preference.
        assert!(outcome.added[0].convert_to_mp3);
    }

    /// One unusable input must not cost the rest of the batch their place
    /// in the queue.
    #[tokio::test]
    async fn skips_input_with_no_video_id_and_queues_the_rest() {
        let store = store().await;
        let videos = vec![
            "https://www.youtube.com/playlist?list=PLabc123".to_string(),
            "dQw4w9WgXcQ".to_string(),
        ];

        let outcome = enqueue_videos(
            None,
            &store,
            &videos,
            FormatPreference::Mp3,
            "/tmp/out",
            None,
        )
        .await
        .unwrap();

        assert_eq!(outcome.added.len(), 1);
        assert_eq!(outcome.skipped.len(), 1);
        assert_eq!(
            outcome.skipped[0].video_id,
            "https://www.youtube.com/playlist?list=PLabc123"
        );
    }

    /// Without a Data API client there is no title to be had, so the entry
    /// is queued under its video id and `core::download` corrects it.
    #[tokio::test]
    async fn falls_back_to_the_video_id_as_title() {
        let store = store().await;
        let outcome = enqueue_videos(
            None,
            &store,
            &["dQw4w9WgXcQ".to_string()],
            FormatPreference::Mp3,
            "/tmp/out",
            None,
        )
        .await
        .unwrap();

        assert_eq!(outcome.added[0].title, "dQw4w9WgXcQ");
    }
}
