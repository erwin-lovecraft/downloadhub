//! Inspection of a Netscape `cookies.txt` file before it's handed to
//! yt-dlp, so a file that yt-dlp would quietly ignore can be reported to
//! the user instead.
//!
//! yt-dlp's own feedback here is unusable as a signal: a file whose fields
//! aren't tab-separated parses "successfully" with every entry dropped,
//! announced only as a `WARNING` on stderr — which the app suppresses with
//! `--no-warnings`, and which yt-dlp doesn't emit at all for cookies that
//! are merely expired. Either way the download then fails the bot check as
//! if no cookies had been configured.

use std::path::Path;

/// What a cookies file actually contains, in the terms that decide whether
/// yt-dlp can use it.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct CookieFileReport {
    /// Whether the first line is the magic comment yt-dlp requires. Without
    /// it yt-dlp refuses the file outright ("does not look like a Netscape
    /// format cookies file").
    pub has_netscape_header: bool,
    /// Entries yt-dlp will load: seven tab-separated fields, not expired.
    pub usable_entries: usize,
    /// Lines that look like cookies but aren't seven tab-separated fields —
    /// almost always tabs turned into spaces by copy-pasting. yt-dlp skips
    /// each one with a warning nothing surfaces.
    pub malformed_lines: usize,
    /// Entries whose expiry is in the past. yt-dlp drops these silently.
    pub expired_entries: usize,
    /// Whether any usable entry is a YouTube/Google sign-in cookie. Cookies
    /// without one get past the format check but do nothing for a bot
    /// check, since they don't carry a session.
    pub has_session_cookie: bool,
}

/// Cookie names that carry a signed-in YouTube session. `SID` alone isn't
/// enough for the `__Secure-` variants YouTube moved to, so all are
/// accepted.
const SESSION_COOKIE_NAMES: [&str; 5] = [
    "SID",
    "__Secure-1PSID",
    "__Secure-3PSID",
    "__Secure-1PSIDTS",
    "__Secure-3PSIDTS",
];

impl CookieFileReport {
    /// User-facing reasons this file won't do what the user expects, worst
    /// first. Empty means yt-dlp can use it — which still isn't proof
    /// YouTube accepts it; only an actual request tells you that.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if !self.has_netscape_header {
            problems.push(
                "The first line isn't `# Netscape HTTP Cookie File`, so yt-dlp refuses the file outright. Export it with a cookies.txt browser extension rather than copying cookies by hand."
                    .to_string(),
            );
        }
        if self.malformed_lines > 0 {
            problems.push(format!(
                "{} line(s) aren't seven TAB-separated fields, so yt-dlp skips them — usually the tabs became spaces on the way into the file. Save the export as-is instead of copying it through an editor.",
                self.malformed_lines
            ));
        }
        if self.usable_entries == 0 {
            problems.push(
                "No usable cookies left in the file, so yt-dlp runs as if signed out.".to_string(),
            );
        } else if !self.has_session_cookie {
            problems.push(
                "No signed-in session cookie (SID / __Secure-*PSID) here, so these cookies won't get past a bot check. Export them from a tab where you're signed in to YouTube."
                    .to_string(),
            );
        }
        if self.expired_entries > 0 {
            problems.push(format!(
                "{} cookie(s) have already expired and are dropped. Export a fresh file.",
                self.expired_entries
            ));
        }
        problems
    }
}

/// Parses `path` the way yt-dlp does, without spawning it. `now_secs` is
/// the current Unix time, passed in so expiry classification is testable.
pub fn inspect_cookie_file(path: &Path, now_secs: i64) -> std::io::Result<CookieFileReport> {
    Ok(inspect_cookie_text(
        &std::fs::read_to_string(path)?,
        now_secs,
    ))
}

fn inspect_cookie_text(text: &str, now_secs: i64) -> CookieFileReport {
    let mut report = CookieFileReport {
        has_netscape_header: text
            .lines()
            .next()
            .map(|l| l.trim().to_ascii_lowercase())
            .is_some_and(|l| {
                l.starts_with("# netscape http cookie file") || l.starts_with("# http cookie file")
            }),
        ..CookieFileReport::default()
    };

    for line in text.lines() {
        // `#HttpOnly_` is a real entry wearing a comment's clothes; every
        // other leading `#` is a comment, and yt-dlp treats it as one.
        let line = match line.strip_prefix("#HttpOnly_") {
            Some(rest) => rest,
            None if line.starts_with('#') || line.trim().is_empty() => continue,
            None => line,
        };

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 7 {
            report.malformed_lines += 1;
            continue;
        }
        // Expiry 0 means a session cookie: no expiry, not "expired in 1970".
        match fields[4].trim().parse::<i64>() {
            Ok(expiry) if expiry != 0 && expiry <= now_secs => {
                report.expired_entries += 1;
                continue;
            }
            Ok(_) => {}
            Err(_) => {
                report.malformed_lines += 1;
                continue;
            }
        }
        report.usable_entries += 1;
        if SESSION_COOKIE_NAMES.contains(&fields[5].trim()) {
            report.has_session_cookie = true;
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;

    fn line(name: &str, expiry: i64) -> String {
        format!(".youtube.com\tTRUE\t/\tTRUE\t{expiry}\t{name}\tvalue\n")
    }

    fn header() -> String {
        "# Netscape HTTP Cookie File\n# This file is generated by yt-dlp.\n\n".to_string()
    }

    #[test]
    fn a_good_export_has_no_problems() {
        let text = header() + &line("SID", NOW + 10_000) + &line("PREF", 0);
        let report = inspect_cookie_text(&text, NOW);
        assert_eq!(report.usable_entries, 2);
        assert!(report.has_session_cookie);
        assert!(report.problems().is_empty(), "{:?}", report.problems());
    }

    #[test]
    fn space_separated_fields_are_counted_as_malformed() {
        // The failure that started this: yt-dlp loads such a file happily
        // and skips every entry, warning on a stream nothing reads.
        let text = header() + ".youtube.com TRUE / TRUE 1800000001 SID value\n";
        let report = inspect_cookie_text(&text, NOW);
        assert_eq!(report.malformed_lines, 1);
        assert_eq!(report.usable_entries, 0);
        assert!(report
            .problems()
            .iter()
            .any(|p| p.contains("TAB-separated")));
    }

    #[test]
    fn a_missing_header_is_reported() {
        let report = inspect_cookie_text(&line("SID", NOW + 10_000), NOW);
        assert!(!report.has_netscape_header);
        assert!(report.problems().iter().any(|p| p.contains("first line")));
    }

    #[test]
    fn expired_cookies_are_counted_separately_from_session_ones() {
        let text = header() + &line("SID", NOW - 1) + &line("PREF", 0);
        let report = inspect_cookie_text(&text, NOW);
        assert_eq!(report.expired_entries, 1);
        // Expiry 0 is a session cookie, not one that expired in 1970.
        assert_eq!(report.usable_entries, 1);
        assert!(!report.has_session_cookie);
    }

    #[test]
    fn http_only_entries_count_as_entries_not_comments() {
        let text = header() + "#HttpOnly_" + &line("__Secure-3PSID", NOW + 10_000);
        let report = inspect_cookie_text(&text, NOW);
        assert_eq!(report.usable_entries, 1);
        assert_eq!(report.malformed_lines, 0);
        assert!(report.has_session_cookie);
    }

    #[test]
    fn cookies_without_a_session_are_called_out() {
        let text = header() + &line("PREF", NOW + 10_000);
        let report = inspect_cookie_text(&text, NOW);
        assert!(report
            .problems()
            .iter()
            .any(|p| p.contains("session cookie")));
    }
}
