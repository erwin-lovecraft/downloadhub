//! `StreamClient`, the business-logic wrapper over a [`StreamProvider`].

use std::path::Path;

use super::config::YtDlpConfig;
use super::models::{
    select_format, FormatPreference, FormatRequest, FormatSummary, ResolvedFormat, VideoDetail,
    AUTO_AUDIO_ITAG,
};
use super::provider::StreamProvider;
use super::StreamError;

/// Wraps whatever [`StreamProvider`] the caller constructs it with (a real
/// yt-dlp process in production, a test double in unit tests) and adds the
/// format-selection logic on top, so call sites don't need to depend on a
/// concrete provider at all.
pub struct StreamClient {
    provider: Box<dyn StreamProvider>,
}

impl StreamClient {
    pub fn new(provider: impl StreamProvider + 'static) -> Self {
        Self {
            provider: Box::new(provider),
        }
    }

    /// Fetches metadata and the full available format/quality list for a
    /// video URL or bare 11-character ID.
    pub async fn get_video_formats(
        &self,
        url_or_id: &str,
        config: &YtDlpConfig,
    ) -> Result<VideoDetail, StreamError> {
        self.provider.get_video(url_or_id, config).await
    }

    /// Fetches a video's formats and picks the one matching `preference`.
    /// Used wherever asking the user to pick an exact itag per video isn't
    /// practical (playlist import, MCP enqueueing, bulk re-format). Returns
    /// [`StreamError::FormatNotFound`] if nothing matches (e.g.
    /// `BestProgressive` on a video YouTube only serves as separate
    /// video/audio DASH streams).
    pub async fn resolve_preferred_format(
        &self,
        url_or_id: &str,
        preference: FormatPreference,
        config: &YtDlpConfig,
    ) -> Result<(VideoDetail, FormatSummary), StreamError> {
        let detail = self.get_video_formats(url_or_id, config).await?;
        let format = select_format(&detail.formats, preference)
            .cloned()
            .ok_or(StreamError::FormatNotFound)?;
        Ok((detail, format))
    }

    /// [`Self::resolve_preferred_format`] reduced to just the fields a queue
    /// row needs — including the `convert_to_mp3` flag, which is a property
    /// of the *preference*, not of the selected stream. The one place that
    /// pairing is decided, so enqueueing and re-formatting can't drift.
    ///
    /// A [`FormatPreference::Mp3`] request never fails for want of a
    /// matching format: with nothing to point at, the entry records
    /// [`AUTO_AUDIO_ITAG`] and the provider picks a stream with audio when
    /// the download actually runs. That covers sources whose format list
    /// this crate can't represent at all — anything whose format ids aren't
    /// numeric — where failing up front would be wrong, since ffmpeg can
    /// still turn whatever arrives into an MP3.
    pub async fn resolve_queue_format(
        &self,
        url_or_id: &str,
        preference: FormatPreference,
        config: &YtDlpConfig,
    ) -> Result<(VideoDetail, ResolvedFormat), StreamError> {
        let detail = self.get_video_formats(url_or_id, config).await?;
        let resolved = match select_format(&detail.formats, preference) {
            Some(format) => ResolvedFormat {
                itag: format.itag,
                quality_label: format.quality_label.clone(),
                convert_to_mp3: preference.convert_to_mp3(),
            },
            None if preference == FormatPreference::Mp3 => ResolvedFormat {
                itag: AUTO_AUDIO_ITAG,
                quality_label: Some("best audio".to_string()),
                convert_to_mp3: true,
            },
            None => return Err(StreamError::FormatNotFound),
        };
        Ok((detail, resolved))
    }

    /// Downloads `request`'s stream for `url_or_id` to the exact path
    /// `dest`. `on_progress` receives raw `(downloaded_bytes, total_bytes)`
    /// calls, unthrottled — the caller decides how often to forward them.
    /// Returns the number of bytes written.
    pub async fn download(
        &self,
        url_or_id: &str,
        request: FormatRequest,
        dest: &Path,
        config: &YtDlpConfig,
        mut on_progress: impl FnMut(u64, u64) + Send,
    ) -> Result<u64, StreamError> {
        self.provider
            .download(url_or_id, request, dest, config, &mut on_progress)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::{BoxFuture, FormatSummary};

    /// Returns whatever format list the test hands it; `download` is never
    /// reached from these tests.
    struct FixedFormats(Vec<FormatSummary>);

    impl StreamProvider for FixedFormats {
        fn get_video<'a>(
            &'a self,
            url_or_id: &'a str,
            _config: &'a YtDlpConfig,
        ) -> BoxFuture<'a, Result<VideoDetail, StreamError>> {
            Box::pin(async move {
                Ok(VideoDetail {
                    video_id: url_or_id.to_string(),
                    title: "Title".to_string(),
                    author: "Author".to_string(),
                    duration_seconds: 60,
                    formats: self.0.clone(),
                })
            })
        }

        fn download<'a>(
            &'a self,
            _url_or_id: &'a str,
            _request: FormatRequest,
            _dest: &'a Path,
            _config: &'a YtDlpConfig,
            _on_progress: &'a mut (dyn FnMut(u64, u64) + Send + 'a),
        ) -> BoxFuture<'a, Result<u64, StreamError>> {
            Box::pin(async { unreachable!("test never downloads") })
        }
    }

    fn client(formats: Vec<FormatSummary>) -> StreamClient {
        StreamClient::new(FixedFormats(formats))
    }

    #[tokio::test]
    async fn an_mp3_request_resolves_to_the_auto_itag_when_no_format_matches() {
        // An empty list is what a source whose format ids aren't numeric
        // looks like by the time it reaches `core` — every format was
        // dropped in conversion, and there's nothing here to point at.
        let (_, resolved) = client(Vec::new())
            .resolve_queue_format("x", FormatPreference::Mp3, &YtDlpConfig::default())
            .await
            .unwrap();
        assert_eq!(resolved.itag, AUTO_AUDIO_ITAG);
        assert!(resolved.convert_to_mp3);
    }

    #[tokio::test]
    async fn a_non_mp3_request_still_fails_when_no_format_matches() {
        let err = client(Vec::new())
            .resolve_queue_format(
                "x",
                FormatPreference::BestAudioOnly,
                &YtDlpConfig::default(),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, StreamError::FormatNotFound));
    }
}
