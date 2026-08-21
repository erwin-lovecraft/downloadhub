//! The `StreamProvider` trait: the seam a concrete yt-dlp wrapper plugs
//! into, keeping `core` free of any dependency on one — the same shape
//! `download::Transcode` uses for ffmpeg.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use super::config::YtDlpConfig;
use super::models::VideoDetail;
use super::StreamError;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Video metadata/format lookup and download via yt-dlp (or any other
/// backend). Object-safe (futures are boxed) so callers can hold a
/// `&dyn StreamProvider`/`Box<dyn StreamProvider>` without making the whole
/// call chain generic. Implemented for a real yt-dlp process by
/// `downloadhub-ytdlp`; `core` never depends on that crate.
pub trait StreamProvider: Send + Sync {
    /// Fetches metadata and the full available format list for a video URL
    /// or bare 11-character id.
    fn get_video<'a>(
        &'a self,
        url_or_id: &'a str,
        config: &'a YtDlpConfig,
    ) -> BoxFuture<'a, Result<VideoDetail, StreamError>>;

    /// Downloads `itag`'s stream for `url_or_id` to the exact path `dest`.
    /// `on_progress` receives raw `(downloaded_bytes, total_bytes)` calls,
    /// unthrottled — the caller decides how often to forward them. Returns
    /// the number of bytes written.
    fn download<'a>(
        &'a self,
        url_or_id: &'a str,
        itag: u32,
        dest: &'a Path,
        config: &'a YtDlpConfig,
        on_progress: &'a mut (dyn FnMut(u64, u64) + Send + 'a),
    ) -> BoxFuture<'a, Result<u64, StreamError>>;
}
