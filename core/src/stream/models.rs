//! The IPC-facing stream models and format-selection logic.

use serde::Serialize;

/// One downloadable stream variant (progressive or adaptive).
#[derive(Debug, Clone, Serialize)]
pub struct FormatSummary {
    pub itag: u32,
    /// File extension yt-dlp reports for this stream, e.g. `mp4`, `webm`,
    /// `m4a`.
    pub ext: String,
    /// Human label for video formats, e.g. `720p`, `1080p`.
    pub quality_label: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub bitrate: Option<u64>,
    pub content_length_bytes: Option<u64>,
    pub has_video: bool,
    pub has_audio: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoDetail {
    pub video_id: String,
    pub title: String,
    pub author: String,
    pub duration_seconds: u64,
    /// In whatever order yt-dlp's format list returns them (roughly
    /// ascending quality) — callers needing a specific pick use
    /// `select_format`/`FormatPreference` rather than relying on order.
    pub formats: Vec<FormatSummary>,
}

impl VideoDetail {
    /// Looks up one specific stream by itag, e.g. to re-check a queue
    /// entry's chosen format is still offered before downloading it.
    pub fn format_by_itag(&self, itag: u32) -> Option<&FormatSummary> {
        self.formats.iter().find(|f| f.itag == itag)
    }
}

/// A quality shortcut for operations that can't reasonably ask the user to
/// pick an exact itag per video, since the available itags vary video to
/// video: playlist import, MCP enqueueing, and bulk re-format of queue
/// entries. Also used as the persisted default quality in `core::settings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatPreference {
    /// Highest-resolution format that has both video and audio in one
    /// stream. Deliberately doesn't fall back to a video-only format if no
    /// progressive one exists — silently producing a video with no sound
    /// would violate what the user asked for.
    #[default]
    BestProgressive,
    /// Highest-bitrate audio-only format, kept in its original container.
    BestAudioOnly,
    /// Audio, transcoded to MP3 after download (`convert_to_mp3`). Selects
    /// [`MP3_SOURCE_ITAG`] when offered, since the conversion is
    /// lossy-to-lossy and wants the best universal source, then degrades
    /// through any other audio-only stream to the cheapest muxed one — see
    /// `mp3_source`. Unlike the other two preferences it is never allowed
    /// to fail for want of a matching format: the fallback of last resort
    /// is [`AUTO_AUDIO_ITAG`], leaving the pick to the provider.
    Mp3,
}

impl FormatPreference {
    /// Whether an entry queued under this preference should be transcoded
    /// to MP3 once downloaded — the `convert_to_mp3` flag on the queue row.
    pub fn convert_to_mp3(self) -> bool {
        matches!(self, FormatPreference::Mp3)
    }
}

/// itag 140 is the standard 128 kbps AAC (m4a) audio-only stream, present
/// on virtually every video. Preferred as the MP3 transcode source over
/// itag 139 (~48 kbps HE-AAC), whose low quality would compound with the
/// lossy-to-lossy conversion.
pub const MP3_SOURCE_ITAG: u32 = 140;

/// Stand-in itag on an MP3 queue entry whose source stream is left for the
/// provider to pick at download time. Not a real itag (YouTube's start at
/// 5): it means "any stream with audio", the fallback for sources whose
/// format list this crate can't represent — non-numeric format ids, or a
/// list that arrived empty. See [`FormatFallback::AnyAudio`].
pub const AUTO_AUDIO_ITAG: u32 = 0;

/// What to ask a [`StreamProvider`](super::StreamProvider) to download: the
/// exact stream a queue entry recorded, plus how far the provider may
/// deviate when that stream isn't on offer. Keeping the deviation semantic
/// (rather than passing a yt-dlp selector string) leaves *how* to fall back
/// to the provider, the same way the rest of this seam works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatRequest {
    pub itag: u32,
    pub fallback: FormatFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormatFallback {
    /// Download that exact itag or fail. What a user who picked a specific
    /// quality asked for — quietly substituting another one would not be.
    #[default]
    Exact,
    /// Any stream carrying audio will do. Only for entries headed into the
    /// MP3 transcode, where the source stream is an implementation detail:
    /// ffmpeg re-encodes it and drops any video track (`-vn`), so which one
    /// it was doesn't survive into the file the user asked for.
    AnyAudio,
}

impl FormatRequest {
    pub fn exact(itag: u32) -> Self {
        Self {
            itag,
            fallback: FormatFallback::Exact,
        }
    }

    pub fn any_audio(itag: u32) -> Self {
        Self {
            itag,
            fallback: FormatFallback::AnyAudio,
        }
    }
}

/// What a [`FormatPreference`] resolved to for one specific video: the
/// exact itag to fetch plus the queue-row fields that go with it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ResolvedFormat {
    pub itag: u32,
    pub quality_label: Option<String>,
    pub convert_to_mp3: bool,
}

/// Picks the format matching `preference` from an already-fetched list, or
/// `None` if nothing qualifies. Pure/no I/O so it's unit-testable without a
/// real video lookup.
pub(crate) fn select_format(
    formats: &[FormatSummary],
    preference: FormatPreference,
) -> Option<&FormatSummary> {
    match preference {
        FormatPreference::BestProgressive => formats
            .iter()
            .filter(|f| f.has_video && f.has_audio)
            .max_by_key(|f| f.height.unwrap_or(0)),
        FormatPreference::BestAudioOnly => best_audio_only(formats),
        FormatPreference::Mp3 => mp3_source(formats),
    }
}

fn best_audio_only(formats: &[FormatSummary]) -> Option<&FormatSummary> {
    formats
        .iter()
        .filter(|f| f.has_audio && !f.has_video)
        .max_by_key(|f| f.bitrate.unwrap_or(0))
}

/// The MP3 transcode source, in descending order of preference:
/// [`MP3_SOURCE_ITAG`], any other audio-only stream, and finally the
/// *cheapest* muxed stream. The last tier is what keeps videos YouTube
/// serves without a separate audio track — live streams and anything that
/// came back as a progressive-only list — out of "no format matched":
/// ffmpeg's `-vn` discards the video track, so a muxed stream converts to
/// MP3 just as well. It only costs the bandwidth of the video being thrown
/// away, hence the smallest one rather than the best.
fn mp3_source(formats: &[FormatSummary]) -> Option<&FormatSummary> {
    formats
        .iter()
        .find(|f| f.itag == MP3_SOURCE_ITAG && f.has_audio && !f.has_video)
        .or_else(|| best_audio_only(formats))
        .or_else(|| cheapest_muxed(formats))
}

fn cheapest_muxed(formats: &[FormatSummary]) -> Option<&FormatSummary> {
    formats
        .iter()
        .filter(|f| f.has_audio && f.has_video)
        .min_by_key(|f| (f.height.unwrap_or(u32::MAX), f.bitrate.unwrap_or(u64::MAX)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(
        itag: u32,
        has_video: bool,
        has_audio: bool,
        height: Option<u32>,
        bitrate: Option<u64>,
    ) -> FormatSummary {
        FormatSummary {
            itag,
            ext: "mp4".to_string(),
            quality_label: None,
            width: None,
            height,
            fps: None,
            bitrate,
            content_length_bytes: None,
            has_video,
            has_audio,
        }
    }

    #[test]
    fn best_progressive_picks_highest_resolution_progressive_format() {
        let formats = vec![
            format(18, true, true, Some(360), None),
            format(22, true, true, Some(720), None),
            // Higher resolution but video-only -- must not be picked for
            // BestProgressive, or the user would silently get no audio.
            format(137, true, false, Some(1080), None),
        ];
        let picked = select_format(&formats, FormatPreference::BestProgressive).unwrap();
        assert_eq!(picked.itag, 22);
    }

    #[test]
    fn best_progressive_returns_none_when_no_progressive_format_exists() {
        let formats = vec![
            format(137, true, false, Some(1080), None),
            format(140, false, true, None, Some(128_000)),
        ];
        assert!(select_format(&formats, FormatPreference::BestProgressive).is_none());
    }

    #[test]
    fn best_audio_only_picks_highest_bitrate_audio_only_format() {
        let formats = vec![
            format(18, true, true, Some(360), Some(96_000)),
            format(139, false, true, None, Some(48_000)),
            format(140, false, true, None, Some(128_000)),
        ];
        let picked = select_format(&formats, FormatPreference::BestAudioOnly).unwrap();
        assert_eq!(picked.itag, 140);
    }

    #[test]
    fn mp3_prefers_the_standard_source_itag_over_a_higher_bitrate_one() {
        let formats = vec![
            format(251, false, true, None, Some(160_000)),
            format(MP3_SOURCE_ITAG, false, true, None, Some(128_000)),
        ];
        let picked = select_format(&formats, FormatPreference::Mp3).unwrap();
        assert_eq!(picked.itag, MP3_SOURCE_ITAG);
    }

    #[test]
    fn mp3_falls_back_to_best_audio_only_when_the_source_itag_is_absent() {
        let formats = vec![
            format(18, true, true, Some(360), Some(96_000)),
            format(139, false, true, None, Some(48_000)),
            format(251, false, true, None, Some(160_000)),
        ];
        let picked = select_format(&formats, FormatPreference::Mp3).unwrap();
        assert_eq!(picked.itag, 251);
    }

    #[test]
    fn mp3_falls_back_to_the_cheapest_muxed_format_when_no_audio_only_one_exists() {
        // What a live stream or an HLS-only extraction looks like: nothing
        // is audio-only, but ffmpeg can still strip the video track. The
        // video is discarded, so the smallest stream is the right one.
        let formats = vec![
            format(22, true, true, Some(720), Some(1_500_000)),
            format(18, true, true, Some(360), Some(600_000)),
            format(137, true, false, Some(1080), Some(3_000_000)),
        ];
        let picked = select_format(&formats, FormatPreference::Mp3).unwrap();
        assert_eq!(picked.itag, 18);
    }

    #[test]
    fn mp3_returns_none_only_when_nothing_carries_audio_at_all() {
        let formats = vec![format(137, true, false, Some(1080), Some(3_000_000))];
        assert!(select_format(&formats, FormatPreference::Mp3).is_none());
        assert!(select_format(&[], FormatPreference::Mp3).is_none());
    }

    #[test]
    fn only_mp3_sets_the_conversion_flag() {
        assert!(FormatPreference::Mp3.convert_to_mp3());
        assert!(!FormatPreference::BestAudioOnly.convert_to_mp3());
        assert!(!FormatPreference::BestProgressive.convert_to_mp3());
    }
}
