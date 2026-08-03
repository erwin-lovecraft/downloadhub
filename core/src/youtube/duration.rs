//! Parser for the ISO 8601 durations `contentDetails.duration` returns.

pub fn parse_iso8601(s: &str) -> Option<u64> {
    let s = s.strip_prefix('P')?;
    let (date_part, time_part) = match s.split_once('T') {
        Some((d, t)) => (d, Some(t)),
        None => (s, None),
    };

    let mut seconds = parse_component(date_part, 'D')? * 86_400;
    if let Some(time_part) = time_part {
        let (rest, hours) = take_component(time_part, 'H')?;
        let (rest, minutes) = take_component(rest, 'M')?;
        let (rest, secs) = take_component(rest, 'S')?;
        if !rest.is_empty() {
            return None;
        }
        seconds += hours * 3_600 + minutes * 60 + secs;
    }
    Some(seconds)
}

/// Parses a whole string as a single `<number><unit>` component, or `0` if
/// the string is empty.
fn parse_component(s: &str, unit: char) -> Option<u64> {
    if s.is_empty() {
        return Some(0);
    }
    let n = s.strip_suffix(unit)?;
    n.parse().ok()
}

/// Pulls a leading `<number><unit>` off `s` if present, returning the
/// remainder and the parsed value (`0` if the unit isn't present at all).
fn take_component(s: &str, unit: char) -> Option<(&str, u64)> {
    match s.find(unit) {
        Some(idx) => {
            let n: u64 = s[..idx].parse().ok()?;
            Some((&s[idx + 1..], n))
        }
        None => Some((s, 0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_and_seconds() {
        assert_eq!(parse_iso8601("PT4M13S"), Some(4 * 60 + 13));
    }

    #[test]
    fn hours_minutes() {
        assert_eq!(parse_iso8601("PT1H2M"), Some(3600 + 120));
    }

    #[test]
    fn seconds_only() {
        assert_eq!(parse_iso8601("PT45S"), Some(45));
    }

    #[test]
    fn zero_duration() {
        assert_eq!(parse_iso8601("P0D"), Some(0));
    }

    #[test]
    fn hours_minutes_seconds() {
        assert_eq!(parse_iso8601("PT2H5M9S"), Some(2 * 3600 + 5 * 60 + 9));
    }

    #[test]
    fn invalid_input() {
        assert_eq!(parse_iso8601("not a duration"), None);
        assert_eq!(parse_iso8601(""), None);
    }
}
