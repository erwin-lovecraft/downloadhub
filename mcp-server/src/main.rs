//! MCP server binary (Phase 3): exposes downloadhub to external AI agents
//! over stdio.
//!
//! Read-only tools (`search_videos`, `get_video_formats`, `list_queue`)
//! execute directly. Tools that would mutate the queue or start a download
//! (`add_to_queue`, `start_download`, `download_all`) never execute here —
//! they record a *pending agent action* (`core::agent`) in the shared
//! queue database, which the running desktop app surfaces for explicit
//! user approval before anything happens. This binary must never trigger
//! downloads unattended.
//!
//! Registration with Claude Desktop / Gemini CLI / Codex is documented in
//! `docs/MCP_SETUP.md`.

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerInfo},
    service::{RequestContext, RoleServer},
    tool, tool_handler, tool_router,
    transport::stdio,
    ServerHandler, ServiceExt,
};

use downloadhub_core::agent::AgentActionRequest;
use downloadhub_core::queue::{NewQueueEntry, QueueStatus, QueueStore};
use downloadhub_core::settings;
use downloadhub_core::stream::StreamClient;
use downloadhub_core::youtube::YoutubeClient;

const INSTRUCTIONS: &str = "Search YouTube and manage downloadhub's download queue. \
Read-only tools (search_videos, get_video_formats, list_queue) return results directly. \
Tools that change the queue or start downloads (add_to_queue, start_download, download_all) \
do NOT execute immediately: they create a pending action that the user must explicitly \
approve inside the running DownloadHub desktop app. After calling one, tell the user to \
open DownloadHub and approve or reject the request, then check list_queue to see the result.";

#[derive(Clone)]
struct DownloadHub {
    stream_client: Arc<StreamClient>,
    queue_store: Arc<QueueStore>,
    settings_path: PathBuf,
    youtube_api_key: Option<String>,
    tool_router: ToolRouter<Self>,
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
    #[schemars(description = "YouTube video URL or bare 11-character video id")]
    video: String,
    #[schemars(description = "Stream format itag to download, as returned by get_video_formats")]
    itag: u32,
    #[schemars(
        description = "Destination folder. Omit to use the user's default output folder from DownloadHub's settings."
    )]
    output_path: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct StartDownloadParams {
    #[schemars(description = "Queue entry id, as returned by list_queue or add_to_queue")]
    queue_id: i64,
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
        let settings = settings::load(&self.settings_path)
            .await
            .map_err(|e| format!("failed to read DownloadHub settings: {e}"))?;
        if settings.mcp_enabled {
            Ok(())
        } else {
            Err(
                "AI agent access is disabled in DownloadHub's settings. Ask the user to enable \
                 'Allow AI agent access (MCP server)' in the app's Settings dialog."
                    .to_string(),
            )
        }
    }

    /// The MCP client's self-reported name, recorded on pending actions so
    /// the approval prompt can say who is asking. Display-only.
    fn requested_by(context: &RequestContext<RoleServer>) -> Option<String> {
        context
            .peer
            .peer_info()
            .map(|info| info.client_info.name.clone())
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
        description = "List every downloadable stream format (itag, mime type, resolution, size, video/audio flags) for a YouTube video. Read-only; needs no configuration."
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
        description = "Request adding a video (in a specific format) to the download queue. Does NOT add it directly: creates a pending action the user must approve in the DownloadHub desktop app."
    )]
    async fn add_to_queue(
        &self,
        Parameters(params): Parameters<AddToQueueParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<String, String> {
        self.ensure_enabled().await?;

        let output_path = match params.output_path {
            Some(path) if !path.trim().is_empty() => path,
            _ => {
                let settings = settings::load(&self.settings_path)
                    .await
                    .map_err(|e| format!("failed to read DownloadHub settings: {e}"))?;
                settings.default_output_path.ok_or_else(|| {
                    "no output_path given and the user has no default output folder configured; \
                     pass output_path or ask the user to set a default in DownloadHub's settings"
                        .to_string()
                })?
            }
        };

        // Resolve the video ourselves rather than trusting agent-supplied
        // title/quality strings, so the approval prompt the user sees
        // describes the actual video, and a bogus itag fails here with the
        // real options instead of surfacing later as a failed download.
        let detail = self
            .stream_client
            .get_video_formats(&params.video)
            .await
            .map_err(|e| e.to_string())?;
        let format = detail
            .formats
            .iter()
            .find(|f| f.itag == params.itag)
            .ok_or_else(|| {
                format!(
                    "itag {} is not offered for this video; available itags: {}",
                    params.itag,
                    detail
                        .formats
                        .iter()
                        .map(|f| f.itag.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;

        let request = AgentActionRequest::AddToQueue {
            entry: NewQueueEntry {
                video_id: detail.video_id.clone(),
                title: detail.title.clone(),
                itag: format.itag,
                quality_label: format
                    .quality_label
                    .clone()
                    .or_else(|| format.quality.clone()),
                output_path,
            },
        };
        self.submit_for_approval(request, &context).await
    }

    #[tool(
        description = "Request starting the download of an existing queue entry. Does NOT start it directly: creates a pending action the user must approve in the DownloadHub desktop app."
    )]
    async fn start_download(
        &self,
        Parameters(params): Parameters<StartDownloadParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<String, String> {
        self.ensure_enabled().await?;

        let entry = self
            .queue_store
            .get_entry(params.queue_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                format!(
                    "queue entry {} not found; call list_queue to see current entries",
                    params.queue_id
                )
            })?;
        if entry.status == QueueStatus::Downloading {
            return Err(format!(
                "queue entry {} is already downloading",
                params.queue_id
            ));
        }

        let request = AgentActionRequest::StartDownload {
            queue_id: entry.id,
            title: entry.title,
        };
        self.submit_for_approval(request, &context).await
    }

    #[tool(
        description = "Request downloading every queued entry, one at a time. Does NOT start anything directly: creates a pending action the user must approve in the DownloadHub desktop app."
    )]
    async fn download_all(&self, context: RequestContext<RoleServer>) -> Result<String, String> {
        self.ensure_enabled().await?;

        let queued = self
            .queue_store
            .list_entries()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|e| e.status == QueueStatus::Queued)
            .count();
        if queued == 0 {
            return Err(
                "the queue has no entries in 'queued' status; add entries first via add_to_queue"
                    .to_string(),
            );
        }

        self.submit_for_approval(AgentActionRequest::DownloadAll, &context)
            .await
    }

    /// Records `request` as a pending agent action and describes the
    /// approval step in the tool result. This is the only way any mutating
    /// tool "executes" in this process.
    async fn submit_for_approval(
        &self,
        request: AgentActionRequest,
        context: &RequestContext<RoleServer>,
    ) -> Result<String, String> {
        let action = self
            .queue_store
            .add_agent_action(request, Self::requested_by(context))
            .await
            .map_err(|e| e.to_string())?;
        to_json(&serde_json::json!({
            "pending_action": action,
            "status": "awaiting_user_approval",
            "note": "This request was recorded but will NOT run until the user approves it in \
                     the DownloadHub desktop app (which must be open). Tell the user to review \
                     it there; afterwards, list_queue reflects the outcome.",
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
