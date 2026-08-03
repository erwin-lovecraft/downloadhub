//! Raw YouTube API wire shapes and their conversion into the public models.

use serde::Deserialize;

use super::duration;
use super::models::VideoSummary;

#[derive(Debug, Deserialize)]
pub(crate) struct SearchListResponse {
    #[serde(default)]
    pub(crate) items: Vec<SearchListItem>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchListItem {
    id: SearchListItemId,
    snippet: SearchListSnippet,
}

impl SearchListItem {
    /// `None` for results that aren't videos (channels, playlists).
    /// Duration is left unset; `videos.list` fills it in afterwards.
    pub(crate) fn into_summary(self) -> Option<VideoSummary> {
        let video_id = self.id.video_id?;
        Some(VideoSummary {
            video_id,
            title: self.snippet.title,
            channel_title: self.snippet.channel_title,
            thumbnail_url: pick_thumbnail(self.snippet.thumbnails),
            published_at: self.snippet.published_at,
            duration_seconds: None,
        })
    }
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
pub(crate) struct PlaylistItemsResponse {
    #[serde(default)]
    pub(crate) items: Vec<PlaylistItem>,
    #[serde(rename = "nextPageToken")]
    pub(crate) next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PlaylistItem {
    snippet: PlaylistItemSnippet,
}

impl PlaylistItem {
    /// `None` for deleted/private videos (no resolvable video id).
    pub(crate) fn into_summary(self) -> Option<VideoSummary> {
        let video_id = self.snippet.resource_id.video_id?;
        Some(VideoSummary {
            video_id,
            title: self.snippet.title,
            channel_title: self
                .snippet
                .video_owner_channel_title
                .unwrap_or(self.snippet.channel_title),
            thumbnail_url: pick_thumbnail(self.snippet.thumbnails),
            published_at: self.snippet.published_at,
            duration_seconds: None,
        })
    }
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

fn pick_thumbnail(thumbnails: Thumbnails) -> Option<String> {
    thumbnails.medium.or(thumbnails.default).map(|t| t.url)
}

#[derive(Debug, Deserialize)]
pub(crate) struct VideosListResponse {
    #[serde(default)]
    pub(crate) items: Vec<VideosListItem>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct VideosListItem {
    id: String,
    #[serde(rename = "contentDetails")]
    content_details: Option<ContentDetails>,
}

impl VideosListItem {
    /// The video id and its parsed duration (`None` when contentDetails is
    /// absent — e.g. a live stream in progress — or unparseable).
    pub(crate) fn into_duration(self) -> (String, Option<u64>) {
        let seconds = self
            .content_details
            .and_then(|cd| duration::parse_iso8601(&cd.duration));
        (self.id, seconds)
    }
}

#[derive(Debug, Deserialize)]
struct ContentDetails {
    duration: String,
}
