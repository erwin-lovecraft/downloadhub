//! Output file naming: maps an entry's title and format to the destination
//! path on disk.
//!
//! `y7dl` doesn't mux DASH streams, so a video-only or audio-only format is
//! saved to its own clearly-labeled file (`<title>.video.<ext>` /
//! `<title>.audio.<ext>`); a progressive format (both tracks in one stream)
//! is saved as `<title>.<ext>` — see `docs/ARCHITECTURE.md`.

use std::path::PathBuf;

pub(crate) fn destination_path(output_folder: &str, title: &str, format: &y7dl::Format) -> PathBuf {
    let suffix = if format.is_video() && format.has_audio() {
        ""
    } else if format.is_video() {
        ".video"
    } else {
        ".audio"
    };
    let ext = extension_for(&format.mime_type);
    let mut path = PathBuf::from(output_folder);
    path.push(format!("{}{suffix}.{ext}", sanitize_filename(title)));
    path
}

/// The final artifact of an MP3 conversion. No `.audio` marker — unlike a
/// bare DASH stream, the mp3 is exactly the file the user asked for.
pub(crate) fn mp3_destination_path(output_folder: &str, title: &str) -> PathBuf {
    let mut path = PathBuf::from(output_folder);
    path.push(format!("{}.mp3", sanitize_filename(title)));
    path
}

/// Maps a format's mime type (e.g. `video/mp4; codecs="avc1..."`) to a file
/// extension. Falls back to `bin` for anything unrecognized rather than
/// guessing.
fn extension_for(mime_type: &str) -> &'static str {
    match mime_type.split(';').next().unwrap_or(mime_type).trim() {
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "audio/mp4" => "m4a",
        "audio/webm" => "webm",
        _ => "bin",
    }
}

/// Strips characters invalid in Windows/Unix filenames and caps length well
/// under any filesystem's path-component limit.
fn sanitize_filename(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    let truncated: String = trimmed.chars().take(150).collect();
    if truncated.is_empty() || truncated.chars().all(|c| c == '_') {
        "video".to_string()
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_filename_replaces_invalid_characters() {
        assert_eq!(
            sanitize_filename("a/b\\c:d*e?f\"g<h>i|j"),
            "a_b_c_d_e_f_g_h_i_j"
        );
    }

    #[test]
    fn sanitize_filename_falls_back_when_empty() {
        assert_eq!(sanitize_filename("   "), "video");
        assert_eq!(sanitize_filename("///"), "video");
    }

    #[test]
    fn sanitize_filename_truncates_long_titles() {
        let long = "a".repeat(300);
        assert_eq!(sanitize_filename(&long).len(), 150);
    }

    #[test]
    fn extension_for_maps_known_mime_types() {
        assert_eq!(extension_for(r#"video/mp4; codecs="avc1.42001E""#), "mp4");
        assert_eq!(extension_for(r#"video/webm; codecs="vp9""#), "webm");
        assert_eq!(extension_for(r#"audio/mp4; codecs="mp4a.40.2""#), "m4a");
        assert_eq!(extension_for(r#"audio/webm; codecs="opus""#), "webm");
        assert_eq!(extension_for("application/octet-stream"), "bin");
    }

    fn format(mime_type: &str, audio_quality: Option<&str>) -> y7dl::Format {
        y7dl::Format {
            itag: 0,
            url: None,
            mime_type: mime_type.to_string(),
            bitrate: None,
            average_bitrate: None,
            width: None,
            height: None,
            fps: None,
            content_length: None,
            quality: None,
            quality_label: None,
            audio_quality: audio_quality.map(str::to_string),
            audio_sample_rate: None,
            audio_channels: None,
            approx_duration_ms: None,
            signature_cipher: None,
        }
    }

    #[test]
    fn destination_path_is_bare_for_progressive_formats() {
        let f = format("video/mp4; codecs=\"avc1\"", Some("AUDIO_QUALITY_MEDIUM"));
        let path = destination_path("C:/out", "My Title", &f);
        assert_eq!(path, PathBuf::from("C:/out").join("My Title.mp4"));
    }

    #[test]
    fn destination_path_labels_video_only_formats() {
        let f = format("video/webm; codecs=\"vp9\"", None);
        let path = destination_path("C:/out", "My Title", &f);
        assert_eq!(path, PathBuf::from("C:/out").join("My Title.video.webm"));
    }

    #[test]
    fn destination_path_labels_audio_only_formats() {
        let f = format(
            "audio/mp4; codecs=\"mp4a.40.2\"",
            Some("AUDIO_QUALITY_MEDIUM"),
        );
        let path = destination_path("C:/out", "My Title", &f);
        assert_eq!(path, PathBuf::from("C:/out").join("My Title.audio.m4a"));
    }

    #[test]
    fn mp3_destination_path_has_no_audio_marker() {
        let path = mp3_destination_path("C:/out", "My: Title?");
        assert_eq!(path, PathBuf::from("C:/out").join("My_ Title_.mp3"));
    }
}
