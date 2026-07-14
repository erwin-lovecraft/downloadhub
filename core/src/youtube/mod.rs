//! YouTube Data API v3 client: keyword search and video metadata.
//!
//! Calls the REST API directly with `reqwest` + `serde` rather than the
//! generated `google-youtube3` crate. `search.list`/`videos.list` are two
//! plain API-key-authenticated GET requests with a small response shape;
//! the generated client would additionally pull in `yup-oauth2` and an
//! authenticator/hyper-connector setup that's pure friction for calls that
//! don't need user auth at all. See `docs/ARCHITECTURE.md`.

mod duration;

use serde::Deserialize;
use std::collections::HashMap;

const SEARCH_URL: &str = "https://www.googleapis.com/youtube/v3/search";
const VIDEOS_URL: &str = "https://www.googleapis.com/youtube/v3/videos";

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
                let thumbnail_url = item
                    .snippet
                    .thumbnails
                    .medium
                    .or(item.snippet.thumbnails.default)
                    .map(|t| t.url);
                Some(VideoSummary {
                    video_id,
                    title: item.snippet.title,
                    channel_title: item.snippet.channel_title,
                    thumbnail_url,
                    published_at: item.snippet.published_at,
                    duration_seconds: None,
                })
            })
            .collect();

        if results.is_empty() {
            return Ok(results);
        }

        let ids = results
            .iter()
            .map(|r| r.video_id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let videos_response = self
            .http
            .get(VIDEOS_URL)
            .query(&[("part", "contentDetails"), ("id", ids.as_str()), ("key", &self.api_key)])
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

        for result in &mut results {
            if let Some(seconds) = durations.get(&result.video_id) {
                result.duration_seconds = *seconds;
            }
        }

        Ok(results)
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
