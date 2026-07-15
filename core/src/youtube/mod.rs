//! YouTube Data API v3 client: keyword search, playlist listing, and video
//! metadata.
//!
//! Calls the REST API directly with `reqwest` + `serde` rather than the
//! generated `google-youtube3` crate. `search.list`/`videos.list`/
//! `playlistItems.list` are all plain API-key-authenticated GET requests
//! with a small response shape; the generated client would additionally
//! pull in `yup-oauth2` and an authenticator/hyper-connector setup that's
//! pure friction for calls that don't need user auth at all. See
//! `docs/ARCHITECTURE.md`.

mod duration;

use serde::Deserialize;
use std::collections::HashMap;

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

#[derive(Debug, Clone, serde::Serialize)]
pub struct VideoSummary {
    pub video_id: String,
    pub title: String,
    pub channel_title: String,
    pub thumbnail_url: Option<String>,
    pub published_at: String,
    /// `None` if `videos.list` didn't return contentDetails for this id
    /// (e.g. a live stream in progress) or its duration couldn't be parsed.
    pub duration_seconds: Option<u64>,
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
            .filter_map(|item| {
                let video_id = item.id.video_id?;
                Some(VideoSummary {
                    video_id,
                    title: item.snippet.title,
                    channel_title: item.snippet.channel_title,
                    thumbnail_url: pick_thumbnail(item.snippet.thumbnails),
                    published_at: item.snippet.published_at,
                    duration_seconds: None,
                })
            })
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

            for item in response.items {
                let Some(video_id) = item.snippet.resource_id.video_id else {
                    continue;
                };
                results.push(VideoSummary {
                    video_id,
                    title: item.snippet.title,
                    channel_title: item
                        .snippet
                        .video_owner_channel_title
                        .unwrap_or(item.snippet.channel_title),
                    thumbnail_url: pick_thumbnail(item.snippet.thumbnails),
                    published_at: item.snippet.published_at,
                    duration_seconds: None,
                });
            }

            page_token = response.next_page_token;
            if page_token.is_none() || results.len() >= MAX_PLAYLIST_ITEMS {
                break;
            }
        }

        self.enrich_with_durations(&mut results).await?;
        Ok(results)
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
                .map(|item| {
                    let seconds = item
                        .content_details
                        .and_then(|cd| duration::parse_iso8601(&cd.duration));
                    (item.id, seconds)
                })
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

fn pick_thumbnail(thumbnails: Thumbnails) -> Option<String> {
    thumbnails.medium.or(thumbnails.default).map(|t| t.url)
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

#[derive(Debug, Deserialize)]
struct SearchListResponse {
    #[serde(default)]
    items: Vec<SearchListItem>,
}

#[derive(Debug, Deserialize)]
struct SearchListItem {
    id: SearchListItemId,
    snippet: SearchListSnippet,
}

#[derive(Debug, Deserialize)]
struct SearchListItemId {
    #[serde(rename = "videoId")]
    video_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchListSnippet {
    title: String,
    #[serde(rename = "channelTitle")]
    channel_title: String,
    #[serde(rename = "publishedAt")]
    published_at: String,
    thumbnails: Thumbnails,
}

#[derive(Debug, Deserialize)]
struct PlaylistItemsResponse {
    #[serde(default)]
    items: Vec<PlaylistItem>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlaylistItem {
    snippet: PlaylistItemSnippet,
}

#[derive(Debug, Deserialize)]
struct PlaylistItemSnippet {
    title: String,
    #[serde(rename = "channelTitle")]
    channel_title: String,
    /// The video's own channel, as opposed to `channelTitle` (the
    /// playlist owner's channel). Absent for deleted/private videos.
    #[serde(rename = "videoOwnerChannelTitle")]
    video_owner_channel_title: Option<String>,
    #[serde(rename = "publishedAt")]
    published_at: String,
    thumbnails: Thumbnails,
    #[serde(rename = "resourceId")]
    resource_id: PlaylistItemResourceId,
}

#[derive(Debug, Deserialize)]
struct PlaylistItemResourceId {
    #[serde(rename = "videoId")]
    video_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct Thumbnails {
    default: Option<Thumbnail>,
    medium: Option<Thumbnail>,
}

#[derive(Debug, Deserialize)]
struct Thumbnail {
    url: String,
}

#[derive(Debug, Deserialize)]
struct VideosListResponse {
    #[serde(default)]
    items: Vec<VideosListItem>,
}

#[derive(Debug, Deserialize)]
struct VideosListItem {
    id: String,
    #[serde(rename = "contentDetails")]
    content_details: Option<ContentDetails>,
}

#[derive(Debug, Deserialize)]
struct ContentDetails {
    duration: String,
}

#[cfg(test)]
mod tests {
    use super::*;

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
