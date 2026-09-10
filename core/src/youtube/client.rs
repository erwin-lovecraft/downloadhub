//! `YoutubeClient`: HTTP orchestration against the YouTube Data API.

use std::collections::HashMap;

use serde::Deserialize;

use super::models::VideoSummary;
use super::response::{PlaylistItemsResponse, SearchListResponse, VideosListResponse};

const SEARCH_URL: &str = "https://www.googleapis.com/youtube/v3/search";
const VIDEOS_URL: &str = "https://www.googleapis.com/youtube/v3/videos";
const PLAYLIST_ITEMS_URL: &str = "https://www.googleapis.com/youtube/v3/playlistItems";
/// `videos.list`'s `id` parameter accepts at most 50 comma-separated ids.
const VIDEOS_BATCH_SIZE: usize = 50;
/// Caps how many items a single playlist import fetches (4 pages of the
/// API's max page size), so an enormous playlist can't turn one import
/// into hundreds of API calls / a very long wait.
const MAX_PLAYLIST_ITEMS: usize = 200;

#[derive(Debug, thiserror::Error)]
pub enum YoutubeError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("youtube api error ({status}): {message}")]
    Api { status: u16, message: String },
}

pub struct YoutubeClient {
    api_key: String,
    http: reqwest::Client,
}

impl YoutubeClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            http: reqwest::Client::new(),
        }
    }

    /// Searches videos by keyword and enriches results with duration via a
    /// follow-up `videos.list` call (`search.list` doesn't return it).
    pub async fn search_videos(
        &self,
        query: &str,
        max_results: u32,
    ) -> Result<Vec<VideoSummary>, YoutubeError> {
        let search_response = self
            .http
            .get(SEARCH_URL)
            .query(&[
                ("part", "snippet"),
                ("type", "video"),
                ("q", query),
                ("maxResults", &max_results.to_string()),
                ("key", &self.api_key),
            ])
            .send()
            .await?;
        let search_response: SearchListResponse = parse_response(search_response).await?;

        let mut results: Vec<VideoSummary> = search_response
            .items
            .into_iter()
            .filter_map(|item| item.into_summary())
            .collect();

        self.enrich_with_durations(&mut results).await?;
        Ok(results)
    }

    /// Lists a playlist's videos (metadata + duration), paginating through
    /// `playlistItems.list` up to [`MAX_PLAYLIST_ITEMS`]. Accepts either a
    /// bare playlist id or a playlist/watch URL containing one (see
    /// [`extract_playlist_id`]).
    pub async fn list_playlist_items(
        &self,
        playlist_url_or_id: &str,
    ) -> Result<Vec<VideoSummary>, YoutubeError> {
        let playlist_id = extract_playlist_id(playlist_url_or_id);
        let mut results = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let max_results = (MAX_PLAYLIST_ITEMS - results.len()).min(50);
            let mut query = vec![
                ("part", "snippet".to_string()),
                ("playlistId", playlist_id.clone()),
                ("maxResults", max_results.to_string()),
                ("key", self.api_key.clone()),
            ];
            if let Some(token) = &page_token {
                query.push(("pageToken", token.clone()));
            }

            let response = self
                .http
                .get(PLAYLIST_ITEMS_URL)
                .query(&query)
                .send()
                .await?;
            let response: PlaylistItemsResponse = parse_response(response).await?;

            results.extend(response.items.into_iter().filter_map(|i| i.into_summary()));

            page_token = response.next_page_token;
            if page_token.is_none() || results.len() >= MAX_PLAYLIST_ITEMS {
                break;
            }
        }

        self.enrich_with_durations(&mut results).await?;
        Ok(results)
    }

    /// Looks up just the titles for `video_ids`, keyed by id, via
    /// `videos.list` batched 50 ids per request.
    ///
    /// This is what lets enqueueing skip yt-dlp entirely: a queue row needs
    /// a title, and asking the Data API for a whole batch of them costs one
    /// HTTP round-trip, where resolving them through yt-dlp costs one
    /// process launch *per video* (see `core::enqueue`).
    ///
    /// Ids the API doesn't return — private, deleted, region-blocked, or
    /// simply not a real id — are absent from the map rather than an error;
    /// the caller decides what to do without a title.
    pub async fn fetch_titles(
        &self,
        video_ids: &[String],
    ) -> Result<HashMap<String, String>, YoutubeError> {
        let mut titles = HashMap::new();

        for batch in video_ids.chunks(VIDEOS_BATCH_SIZE) {
            let ids = batch.join(",");
            if ids.is_empty() {
                continue;
            }

            let response = self
                .http
                .get(VIDEOS_URL)
                .query(&[
                    ("part", "snippet"),
                    ("id", ids.as_str()),
                    ("key", &self.api_key),
                ])
                .send()
                .await?;
            let response: VideosListResponse = parse_response(response).await?;

            titles.extend(
                response
                    .items
                    .into_iter()
                    .filter_map(|item| item.into_title()),
            );
        }

        Ok(titles)
    }

    /// Fills in `duration_seconds` for each result via `videos.list`,
    /// batched (its `id` parameter accepts at most 50 ids per call).
    async fn enrich_with_durations(
        &self,
        results: &mut [VideoSummary],
    ) -> Result<(), YoutubeError> {
        for batch in results.chunks_mut(VIDEOS_BATCH_SIZE) {
            let ids = batch
                .iter()
                .map(|r| r.video_id.as_str())
                .collect::<Vec<_>>()
                .join(",");
            if ids.is_empty() {
                continue;
            }

            let videos_response = self
                .http
                .get(VIDEOS_URL)
                .query(&[
                    ("part", "contentDetails"),
                    ("id", ids.as_str()),
                    ("key", &self.api_key),
                ])
                .send()
                .await?;
            let videos_response: VideosListResponse = parse_response(videos_response).await?;

            let durations: HashMap<String, Option<u64>> = videos_response
                .items
                .into_iter()
                .map(|item| item.into_duration())
                .collect();

            for result in batch.iter_mut() {
                if let Some(seconds) = durations.get(&result.video_id) {
                    result.duration_seconds = *seconds;
                }
            }
        }
        Ok(())
    }
}

/// Extracts the 11-character video id from a YouTube URL (`watch?v=`,
/// `youtu.be/<id>`, `/shorts/<id>`, `/embed/<id>`, `/live/<id>`), or returns
/// the input unchanged when it already is a bare id. `None` for anything
/// else — including a non-YouTube URL that happens to carry a `v=`
/// parameter, since queueing a video id lifted out of an unrelated site's
/// URL is a guess, not an extraction.
///
/// Enqueueing needs this because it no longer runs yt-dlp, which used to be
/// what turned a pasted URL into the id a queue row stores.
pub fn extract_video_id(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if is_video_id(trimmed) {
        return Some(trimmed.to_string());
    }

    let url = url::Url::parse(trimmed).ok()?;
    let host = url.host_str()?.trim_start_matches("www.");
    if !matches!(host, "youtube.com" | "m.youtube.com" | "youtu.be") {
        return None;
    }

    let candidate = if host == "youtu.be" {
        url.path_segments()?.next().map(str::to_string)
    } else if let Some((_, value)) = url.query_pairs().find(|(key, _)| key == "v") {
        Some(value.into_owned())
    } else {
        // `/shorts/<id>`, `/embed/<id>`, `/live/<id>` — the id is the
        // segment after the marker, not the first one.
        let mut segments = url.path_segments()?;
        segments
            .find(|s| matches!(*s, "shorts" | "embed" | "live" | "v"))
            .and_then(|_| segments.next())
            .map(str::to_string)
    };

    candidate.filter(|id| is_video_id(id))
}

/// YouTube video ids are exactly 11 characters of the URL-safe base64
/// alphabet. Checking the shape keeps a stray path segment (`/playlist`)
/// from being stored as an id.
fn is_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Extracts a playlist id from a `list=` query parameter if `input` parses
/// as a URL (playlist page or a watch page with a playlist attached);
/// otherwise treats the trimmed input as a bare playlist id already.
fn extract_playlist_id(input: &str) -> String {
    let trimmed = input.trim();
    match url::Url::parse(trimmed) {
        Ok(url) => url
            .query_pairs()
            .find(|(key, _)| key == "list")
            .map(|(_, value)| value.into_owned())
            .unwrap_or_else(|| trimmed.to_string()),
        Err(_) => trimmed.to_string(),
    }
}

async fn parse_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, YoutubeError> {
    let status = response.status();
    if !status.is_success() {
        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "<no body>".to_string());
        return Err(YoutubeError::Api {
            status: status.as_u16(),
            message,
        });
    }
    Ok(response.json::<T>().await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_video_id_from_every_youtube_url_shape() {
        for url in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PL1&index=3",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ?t=42",
            "https://www.youtube.com/shorts/dQw4w9WgXcQ",
            "https://www.youtube.com/embed/dQw4w9WgXcQ",
            "https://www.youtube.com/live/dQw4w9WgXcQ",
            "https://m.youtube.com/watch?v=dQw4w9WgXcQ",
        ] {
            assert_eq!(
                extract_video_id(url).as_deref(),
                Some("dQw4w9WgXcQ"),
                "failed on {url}"
            );
        }
    }

    #[test]
    fn passes_through_a_bare_video_id() {
        assert_eq!(
            extract_video_id("  dQw4w9WgXcQ  ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
    }

    /// A path segment that isn't an id must not be stored as one — the
    /// queue row would then point at a video that doesn't exist.
    #[test]
    fn rejects_input_carrying_no_video_id() {
        for input in [
            "not a video",
            "short",
            "https://www.youtube.com/playlist?list=PLabc123",
            "https://www.youtube.com/results?search_query=lofi",
            // A valid-looking id on a host that isn't YouTube's.
            "https://example.com/watch?v=dQw4w9WgXcQ",
        ] {
            assert_eq!(
                extract_video_id(input),
                None,
                "should not have found an id in {input}"
            );
        }
    }

    #[test]
    fn extracts_playlist_id_from_playlist_url() {
        assert_eq!(
            extract_playlist_id("https://www.youtube.com/playlist?list=PLabc123"),
            "PLabc123"
        );
    }

    #[test]
    fn extracts_playlist_id_from_watch_url_with_playlist() {
        assert_eq!(
            extract_playlist_id("https://www.youtube.com/watch?v=xyz&list=PLabc123&index=2"),
            "PLabc123"
        );
    }

    #[test]
    fn passes_through_bare_playlist_id() {
        assert_eq!(extract_playlist_id("  PLabc123  "), "PLabc123");
    }

    #[test]
    fn falls_back_to_trimmed_input_when_url_has_no_list_param() {
        assert_eq!(
            extract_playlist_id("https://www.youtube.com/watch?v=xyz"),
            "https://www.youtube.com/watch?v=xyz"
        );
    }
}
