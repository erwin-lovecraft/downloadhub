//! Output file naming: maps an entry's title and format to a destination path.

use std::path::PathBuf;

use crate::stream::FormatSummary;

/// `format` is `None` only for an MP3 entry whose source stream the provider
/// picks at download time (see `stream::AUTO_AUDIO_ITAG`): the container
/// isn't known ahead of time, so the file gets `ytdlp`'s own unknown-format
/// extension. It's transient either way — the transcode deletes it.
pub(crate) fn destination_path(
    output_folder: &str,
    title: &str,
    format: Option<&FormatSummary>,
) -> PathBuf {
    let (suffix, ext) = match format {
        Some(f) if f.has_video && f.has_audio => ("", f.ext.as_str()),
        Some(f) if f.has_video => (".video", f.ext.as_str()),
        Some(f) => (".audio", f.ext.as_str()),
        None => (".audio", "bin"),
    };
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

    fn format(ext: &str, has_video: bool, has_audio: bool) -> FormatSummary {
        FormatSummary {
            itag: 0,
            ext: ext.to_string(),
            quality_label: None,
            width: None,
            height: None,
            fps: None,
            bitrate: None,
            content_length_bytes: None,
            has_video,
            has_audio,
        }
    }

    #[test]
    fn destination_path_is_bare_for_progressive_formats() {
        let f = format("mp4", true, true);
        let path = destination_path("C:/out", "My Title", Some(&f));
        assert_eq!(path, PathBuf::from("C:/out").join("My Title.mp4"));
    }

    #[test]
    fn destination_path_labels_video_only_formats() {
        let f = format("webm", true, false);
        let path = destination_path("C:/out", "My Title", Some(&f));
        assert_eq!(path, PathBuf::from("C:/out").join("My Title.video.webm"));
    }

    #[test]
    fn destination_path_labels_audio_only_formats() {
        let f = format("m4a", false, true);
        let path = destination_path("C:/out", "My Title", Some(&f));
        assert_eq!(path, PathBuf::from("C:/out").join("My Title.audio.m4a"));
    }

    #[test]
    fn destination_path_falls_back_when_the_format_is_unknown() {
        let path = destination_path("C:/out", "My Title", None);
        assert_eq!(path, PathBuf::from("C:/out").join("My Title.audio.bin"));
    }

    #[test]
    fn mp3_destination_path_has_no_audio_marker() {
        let path = mp3_destination_path("C:/out", "My: Title?");
        assert_eq!(path, PathBuf::from("C:/out").join("My_ Title_.mp3"));
    }
}
