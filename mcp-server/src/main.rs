//! MCP server binary: exposes downloadhub's search and queue tools over stdio.
//! It deliberately exposes no tool that can start a transfer — see
//! `mcp-server/CLAUDE.md` and `docs/MCP_SETUP.md`.

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
    ServerHandler, ServiceExt,
};

use downloadhub_core::enqueue;
use downloadhub_core::queue::QueueStore;
use downloadhub_core::settings;
use downloadhub_core::stream::{FormatPreference, StreamClient};
use downloadhub_core::youtube::YoutubeClient;

const INSTRUCTIONS: &str = "Search YouTube and fill DownloadHub's download queue. \
Adding to the queue takes effect immediately — no approval step. \
Nothing is downloaded until the user opens the DownloadHub desktop app and clicks \
'Download all'; this server has no tool that can start a transfer, so always tell the \
user to do that once you have queued what they asked for. \
Prefer add_mp3_to_queue for music/audio requests and add_to_queue for video. \
Both accept a LIST of videos: queue everything in one call rather than calling once per video.";

#[derive(Clone)]
struct DownloadHub {
    stream_client: Arc<StreamClient>,
    queue_store: Arc<QueueStore>,
    settings_path: PathBuf,
    youtube_api_key: Option<String>,
    tool_router: ToolRouter<Self>,
}

/// The agent-facing spelling of [`FormatPreference`]. Mirrored here rather
/// than deriving `JsonSchema` on the core type so `core` doesn't take a
/// schemars dependency for one binary's benefit, and so the descriptions
/// can be written for an agent rather than for the UI.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Quality {
    /// Highest resolution with video and audio in a single file.
    #[default]
    BestProgressive,
    /// Highest-bitrate audio-only stream, left in its original container.
    BestAudioOnly,
    /// Audio-only, converted to MP3 after download.
    Mp3,
}

impl From<Quality> for FormatPreference {
    fn from(quality: Quality) -> Self {
        match quality {
            Quality::BestProgressive => FormatPreference::BestProgressive,
            Quality::BestAudioOnly => FormatPreference::BestAudioOnly,
            Quality::Mp3 => FormatPreference::Mp3,
        }
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct SearchVideosParams {
    #[schemars(description = "Keyword query to search YouTube videos for")]
    query: String,
    #[schemars(description = "Maximum number of results (1-25, default 10)")]
    max_results: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct GetVideoFormatsParams {
    #[schemars(description = "YouTube video URL or bare 11-character video id")]
    video: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct AddToQueueParams {
    #[schemars(
        description = "YouTube video URLs or bare 11-character video ids. Pass every video you want queued in this one call."
    )]
    videos: Vec<String>,
    #[schemars(
        description = "Quality to resolve for each video. Defaults to the user's configured default quality."
    )]
    quality: Option<Quality>,
    #[schemars(
        description = "Destination folder. Omit to use the user's default output folder, falling back to their Downloads folder."
    )]
    output_path: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct AddMp3ToQueueParams {
    #[schemars(
        description = "YouTube video URLs or bare 11-character video ids. Pass every track you want queued in this one call."
    )]
    videos: Vec<String>,
    #[schemars(
        description = "Destination folder. Omit to use the user's default output folder, falling back to their Downloads folder."
    )]
    output_path: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct RemoveFromQueueParams {
    #[schemars(description = "Queue entry ids to remove, as returned by list_queue")]
    queue_ids: Vec<i64>,
}

#[tool_router(router = tool_router)]
impl DownloadHub {
    fn new() -> anyhow::Result<Self> {
        let app_data_dir = downloadhub_core::paths::app_data_dir().ok_or_else(|| {
            anyhow::anyhow!(
                "no writable app data directory found; downloadhub has nowhere to keep its queue"
            )
        })?;
        let queue_store = QueueStore::open(&downloadhub_core::paths::queue_db_path(&app_data_dir))?;
        Ok(Self {
            stream_client: Arc::new(StreamClient::new()),
            queue_store: Arc::new(queue_store),
            settings_path: downloadhub_core::paths::settings_path(&app_data_dir),
            youtube_api_key: downloadhub_core::secrets::youtube_api_key(),
            tool_router: Self::tool_router(),
        })
    }

    /// Errors unless the user has MCP access enabled in the app settings.
    /// Re-read on every call (not cached at startup) so toggling the
    /// setting in the running desktop app takes effect immediately.
    async fn ensure_enabled(&self) -> Result<(), String> {
        if self.settings().await?.mcp_enabled {
            Ok(())
        } else {
            Err(
                "AI agent access is disabled in DownloadHub's settings. Ask the user to enable \
                 'Allow AI agent access (MCP server)' in the app's Settings dialog."
                    .to_string(),
            )
        }
    }

    async fn settings(&self) -> Result<settings::AppSettings, String> {
        settings::load(&self.settings_path)
            .await
            .map_err(|e| format!("failed to read DownloadHub settings: {e}"))
    }

    /// The folder queued entries land in: the caller's explicit choice, the
    /// user's configured default, then their Downloads folder. Only a
    /// system with no Downloads folder at all leaves the agent to ask.
    async fn resolve_output_path(&self, requested: Option<String>) -> Result<String, String> {
        if let Some(path) = requested
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
        {
            return Ok(path);
        }
        if let Some(default) = self
            .settings()
            .await?
            .default_output_path
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
        {
            return Ok(default);
        }
        downloadhub_core::paths::downloads_dir()
            .map(|dir| dir.to_string_lossy().into_owned())
            .ok_or_else(|| {
                "no output_path given, and neither a default output folder nor a Downloads \
                 folder could be found; pass output_path explicitly"
                    .to_string()
            })
    }

    #[tool(
        description = "Search YouTube videos by keyword. Returns video id, title, channel, and duration per result. Read-only."
    )]
    async fn search_videos(
        &self,
        Parameters(params): Parameters<SearchVideosParams>,
    ) -> Result<String, String> {
        self.ensure_enabled().await?;
        let api_key = self.youtube_api_key.clone().ok_or_else(|| {
            "YouTube search is not configured for the MCP server: set the YOUTUBE_API_KEY \
             environment variable in this server's MCP registration (see docs/MCP_SETUP.md)."
                .to_string()
        })?;
        let max_results = params.max_results.unwrap_or(10).clamp(1, 25);
        let results = YoutubeClient::new(api_key)
            .search_videos(&params.query, max_results)
            .await
            .map_err(|e| e.to_string())?;
        to_json(&results)
    }

    #[tool(
        description = "List every downloadable stream format (itag, mime type, resolution, size, video/audio flags) for a YouTube video. Read-only. Only needed to inspect what a video offers — add_to_queue resolves formats on its own."
    )]
    async fn get_video_formats(
        &self,
        Parameters(params): Parameters<GetVideoFormatsParams>,
    ) -> Result<String, String> {
        self.ensure_enabled().await?;
        let detail = self
            .stream_client
            .get_video_formats(&params.video)
            .await
            .map_err(|e| e.to_string())?;
        to_json(&detail)
    }

    #[tool(
        description = "List the download queue: every entry with its id, video, format, output folder, and status. Read-only."
    )]
    async fn list_queue(&self) -> Result<String, String> {
        self.ensure_enabled().await?;
        let entries = self
            .queue_store
            .list_entries()
            .await
            .map_err(|e| e.to_string())?;
        to_json(&entries)
    }

    #[tool(
        description = "Add one or more videos to the download queue at the given quality. Takes effect immediately. Does NOT download them: the user starts the queue from the DownloadHub app. Pass all videos in a single call."
    )]
    async fn add_to_queue(
        &self,
        Parameters(params): Parameters<AddToQueueParams>,
    ) -> Result<String, String> {
        self.ensure_enabled().await?;
        let quality = match params.quality {
            Some(quality) => quality.into(),
            None => self.settings().await?.default_quality,
        };
        self.enqueue(params.videos, quality, params.output_path)
            .await
    }

    #[tool(
        description = "Add one or more videos to the download queue as MP3 audio (converted after download). The right tool for music, songs, albums, and podcasts. Takes effect immediately, but downloads nothing: the user starts the queue from the DownloadHub app. Pass all tracks in a single call."
    )]
    async fn add_mp3_to_queue(
        &self,
        Parameters(params): Parameters<AddMp3ToQueueParams>,
    ) -> Result<String, String> {
        self.ensure_enabled().await?;
        self.enqueue(params.videos, FormatPreference::Mp3, params.output_path)
            .await
    }

    #[tool(
        description = "Remove entries from the download queue by id. Use to undo a mistaken add before the user starts downloading."
    )]
    async fn remove_from_queue(
        &self,
        Parameters(params): Parameters<RemoveFromQueueParams>,
    ) -> Result<String, String> {
        self.ensure_enabled().await?;
        for queue_id in &params.queue_ids {
            self.queue_store
                .delete_entry(*queue_id)
                .await
                .map_err(|e| e.to_string())?;
        }
        to_json(&serde_json::json!({
            "removed": params.queue_ids.len(),
            "note": "Entries deleted. A download already running for one of them keeps going \
                     until the user cancels it in the app.",
        }))
    }

    /// The shared body of both add tools: resolve each video's format
    /// against `quality` and insert it. Per-video failures are reported,
    /// not fatal (`core::enqueue`).
    async fn enqueue(
        &self,
        videos: Vec<String>,
        quality: FormatPreference,
        output_path: Option<String>,
    ) -> Result<String, String> {
        if videos.is_empty() {
            return Err("no videos given; pass at least one URL or video id".to_string());
        }
        let output_path = self.resolve_output_path(output_path).await?;

        let outcome = enqueue::enqueue_videos(
            &self.stream_client,
            &self.queue_store,
            &videos,
            quality,
            &output_path,
        )
        .await
        .map_err(|e| e.to_string())?;

        to_json(&serde_json::json!({
            "added": outcome.added,
            "skipped": outcome.skipped,
            "output_path": output_path,
            "note": format!(
                "{} entr{} queued. Nothing is downloading yet — tell the user to open DownloadHub \
                 and click 'Download all' (they can change formats or drop entries there first).",
                outcome.added.len(),
                if outcome.added.len() == 1 { "y" } else { "ies" },
            ),
        }))
    }
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|e| format!("failed to serialize result: {e}"))
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DownloadHub {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build());
        info.server_info =
            Implementation::new("downloadhub", env!("CARGO_PKG_VERSION")).with_title("DownloadHub");
        info.instructions = Some(INSTRUCTIONS.to_string());
        info
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Dev convenience, same as the desktop app: pick up YOUTUBE_API_KEY
    // from a gitignored .env when run from the repo. Real registrations
    // pass env through the MCP client config (docs/MCP_SETUP.md). MCP
    // stdio servers must keep stdout for the protocol, so diagnostics go
    // to stderr only.
    let _ = dotenvy::dotenv();

    let server = DownloadHub::new().inspect_err(|e| eprintln!("mcp-server startup failed: {e}"))?;
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
