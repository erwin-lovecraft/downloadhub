//! The IPC-facing stream models and format-selection logic.
//!
//! `y7dl::Video`/`Format` don't derive `Serialize` (they're built from a
//! `Deserialize`d player response), so these DTOs mirror them in a shape
//! that crosses the Tauri IPC boundary as plain JSON, the same way
//! `core::youtube` shapes `VideoSummary` from the raw API response.

use serde::Serialize;

/// One downloadable stream variant (progressive or adaptive).
#[derive(Debug, Clone, Serialize)]
pub struct FormatSummary {
    pub itag: u32,
    pub mime_type: String,
    /// Coarse quality bucket, e.g. `tiny`, `hd720`.
    pub quality: Option<String>,
    /// Human label for video formats, e.g. `720p`, `1080p60`.
    pub quality_label: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
    pub bitrate: Option<u64>,
    pub content_length_bytes: Option<u64>,
    pub has_video: bool,
    pub has_audio: bool,
}

impl From<y7dl::Format> for FormatSummary {
    fn from(format: y7dl::Format) -> Self {
        Self {
            itag: format.itag,
            mime_type: format.mime_type.clone(),
            quality: format.quality.clone(),
            quality_label: format.quality_label.clone(),
            width: format.width,
            height: format.height,
            fps: format.fps,
            bitrate: format.bitrate,
            content_length_bytes: format.content_length(),
            has_video: format.is_video(),
            has_audio: format.has_audio(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoDetail {
    pub video_id: String,
    pub title: String,
    pub author: String,
    pub duration_seconds: u64,
    /// Progressive formats first, then adaptive (video-only/audio-only) ones
    /// — the order `y7dl::Video::formats` already returns them in.
    pub formats: Vec<FormatSummary>,
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
    /// Audio-only, transcoded to MP3 after download (`convert_to_mp3`).
    /// Selects [`MP3_SOURCE_ITAG`] when offered, since the conversion is
    /// lossy-to-lossy and wants the best universal source.
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
        // Prefer the known-good transcode source, but don't fail the whole
        // request when a video doesn't offer it — any audio-only stream
        // converts to MP3 just as well, only from a different source.
        FormatPreference::Mp3 => formats
            .iter()
            .find(|f| f.itag == MP3_SOURCE_ITAG && f.has_audio && !f.has_video)
            .or_else(|| best_audio_only(formats)),
    }
}

fn best_audio_only(formats: &[FormatSummary]) -> Option<&FormatSummary> {
    formats
        .iter()
        .filter(|f| f.has_audio && !f.has_video)
        .max_by_key(|f| f.bitrate.unwrap_or(0))
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
            mime_type: "video/mp4".to_string(),
            quality: None,
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
    fn mp3_returns_none_when_the_video_has_no_audio_only_format() {
        let formats = vec![format(18, true, true, Some(360), Some(96_000))];
        assert!(select_format(&formats, FormatPreference::Mp3).is_none());
    }

    #[test]
    fn only_mp3_sets_the_conversion_flag() {
        assert!(FormatPreference::Mp3.convert_to_mp3());
        assert!(!FormatPreference::BestAudioOnly.convert_to_mp3());
        assert!(!FormatPreference::BestProgressive.convert_to_mp3());
    }
}
