//! The `Transcode` trait: the seam a concrete transcoder plugs into, keeping
//! `core` free of any dependency on one.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;

/// Boxed error from a transcoder implementation. `core` only displays it
/// (as the queue entry's failure message); it never matches on it.
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Converts a downloaded audio stream to MP3. Object-safe (the future is
/// boxed) so [`DownloadContext`](super::DownloadContext) can hold a
/// `&dyn Transcode` without making the whole download pipeline generic.
pub trait Transcode: Send + Sync {
    /// Transcodes `input` to an MP3 at `output`, overwriting `output` if it
    /// exists. `input` must be left in place — the caller deletes it only
    /// after a successful conversion.
    fn to_mp3<'a>(
        &'a self,
        input: &'a Path,
        output: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<(), BoxError>> + Send + 'a>>;
}
