//! Utility functions for parsing time/date/offset strings into chrono types.

use chrono::{DateTime, FixedOffset, NaiveDateTime, Timelike};
use regex::Regex;
// Only if needed directly here, which it is for parse_offset_string

// Make functions `pub` so they can be used by `extraction.rs` and `logic.rs`.

/// Parses a naive datetime string commonly found in EXIF (YYYY:MM:DD HH:MM:SS[.fff]).
/// Returns the `NaiveDateTime` and a boolean indicating if subseconds were present in the string.
pub fn parse_naive(s: &str) -> Option<(NaiveDateTime, bool)> {
    let formats = [
        ("%Y:%m:%d %H:%M:%S%.f", true),
        ("%Y-%m-%d %H:%M:%S%.f", true),
        ("%Y:%m:%d %H:%M:%S", false),
        ("%Y-%m-%d %H:%M:%S", false),
    ];

    for (fmt, has_subsecs_in_fmt) in formats {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            let parsed_subsecs = has_subsecs_in_fmt && dt.nanosecond() != 0;
            return Some((dt, parsed_subsecs));
        }
    }
    None
}

/// Parses a datetime string with a timezone offset (e.g., file modification date).
pub fn parse_datetime_offset(s: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S%z")
        .ok()
        .or_else(|| DateTime::parse_from_rfc3339(s).ok())
}

/// Parses a datetime string ending in 'Z' indicating UTC.
pub fn parse_datetime_utc_z(s: &str) -> Option<DateTime<chrono::Utc>> {
    // Attempt 1: Handle the specific "YYYY:MM:DD HH:MM:SSZ" format from GPS tags.
    // We treat 'Z' as a literal suffix indicating UTC, not a timezone format code.
    if let Some(s_without_z) = s.strip_suffix('Z')
        && let Ok(naive_dt) = NaiveDateTime::parse_from_str(s_without_z, "%Y:%m:%d %H:%M:%S")
    {
        // If the naive part parses correctly, we explicitly attach the UTC timezone.
        return Some(DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            naive_dt,
            chrono::Utc,
        ));
    }

    // Attempt 2 (Fallback): Try parsing as a standard RFC3339 string.
    // This will correctly handle formats like "2024-05-05T10:00:00Z".
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

/// Parses an offset string like "+02:00", "-0500", or "Z" into offset seconds and the original string.
pub fn parse_offset_string(offset_str: &str) -> Option<(i32, String)> {
    if offset_str == "Z" {
        return Some((0, "Z".to_string()));
    }
    let re_offset = Regex::new(r"^([+-])(\d{2}):?(\d{2})$").ok()?;
    if let Some(caps) = re_offset.captures(offset_str) {
        let sign = if caps.get(1)?.as_str() == "-" { -1 } else { 1 };
        let hours = caps.get(2)?.as_str().parse::<i32>().ok()?;
        let minutes = caps.get(3)?.as_str().parse::<i32>().ok()?;
        if hours > 14 || minutes > 59 {
            return None;
        }
        let total_secs = sign * (hours * 3600 + minutes * 60);
        return Some((total_secs, offset_str.to_string()));
    }
    None
}

/// Adds subsecond precision from a separate numeric EXIF field to a `NaiveDateTime`.
pub fn add_subseconds_from_number(dt: NaiveDateTime, subsec_num: u32) -> NaiveDateTime {
    if subsec_num == 0 {
        return dt;
    }
    let subsec_str = subsec_num.to_string();
    let Ok(num_digits) = u32::try_from(subsec_str.len()) else {
        return dt;
    };
    let nanos = if num_digits <= 9 {
        subsec_num.saturating_mul(10u32.pow(9u32.saturating_sub(num_digits)))
    } else {
        subsec_num % 1_000_000_000
    };
    dt.with_nanosecond(nanos).unwrap_or(dt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    // --- Tests for `parse_naive` ---
    mod parse_naive_tests {
        use super::*;

        #[test]
        fn parses_colon_separated_date() {
            let (dt, has_subsec) = parse_naive("2024:01:01 10:30:00").unwrap();
            assert_eq!(
                dt,
                NaiveDate::from_ymd_opt(2024, 1, 1)
                    .unwrap()
                    .and_hms_opt(10, 30, 0)
                    .unwrap()
            );
            assert!(!has_subsec);
        }

        #[test]
        fn parses_hyphen_separated_date() {
            let (dt, has_subsec) = parse_naive("2024-02-02 11:00:00").unwrap();
            assert_eq!(
                dt,
                NaiveDate::from_ymd_opt(2024, 2, 2)
                    .unwrap()
                    .and_hms_opt(11, 0, 0)
                    .unwrap()
            );
            assert!(!has_subsec);
        }

        #[test]
        fn parses_with_subseconds() {
            let (dt, has_subsec) = parse_naive("2024:03:03 12:00:00.123").unwrap();
            assert_eq!(
                dt,
                NaiveDate::from_ymd_opt(2024, 3, 3)
                    .unwrap()
                    .and_hms_milli_opt(12, 0, 0, 123)
                    .unwrap()
            );
            assert!(has_subsec);
        }

        #[test]
        fn returns_none_for_invalid_format() {
            assert!(parse_naive("not a date").is_none());
            assert!(parse_naive("2024/01/01 10:30:00").is_none());
        }
    }

    // --- Tests for `parse_datetime_offset` ---
    mod parse_datetime_offset_tests {
        use super::*;

        #[test]
        fn parses_exif_offset_format() {
            let dt = parse_datetime_offset("2024:08:08 10:00:00+02:00").unwrap();
            assert_eq!(dt.to_rfc3339(), "2024-08-08T10:00:00+02:00");
        }

        #[test]
        fn parses_rfc3339_fallback() {
            let dt = parse_datetime_offset("2024-09-09T14:30:00-05:00").unwrap();
            assert_eq!(dt.to_rfc3339(), "2024-09-09T14:30:00-05:00");
        }

        #[test]
        fn returns_none_for_missing_offset() {
            assert!(parse_datetime_offset("2024:08:08 10:00:00").is_none());
        }
    }

    // --- Tests for `parse_datetime_utc_z` ---
    mod parse_datetime_utc_z_tests {
        use super::*;

        #[test]
        fn parses_gps_datetime_format_with_z() {
            // This is the critical test that validates the recent fix.
            let dt = parse_datetime_utc_z("2024:05:05 10:00:00Z").unwrap();
            assert_eq!(dt.to_rfc3339(), "2024-05-05T10:00:00+00:00");
        }

        #[test]
        fn parses_rfc3339_utc_format() {
            // This ensures the fallback logic still works.
            let dt = parse_datetime_utc_z("2024-06-06T11:22:33Z").unwrap();
            assert_eq!(dt.to_rfc3339(), "2024-06-06T11:22:33+00:00");
        }

        #[test]
        fn returns_none_if_not_utc() {
            assert!(parse_datetime_utc_z("2024:05:05 10:00:00").is_none());
        }
    }

    // --- Tests for `parse_offset_string` ---
    mod parse_offset_string_tests {
        use super::*;

        #[test]
        fn parses_positive_offset_with_colon() {
            let (secs, s) = parse_offset_string("+02:00").unwrap();
            assert_eq!(secs, 2 * 3600);
            assert_eq!(s, "+02:00");
        }

        #[test]
        fn parses_negative_offset_without_colon() {
            let (secs, s) = parse_offset_string("-0500").unwrap();
            assert_eq!(secs, -5 * 3600);
            assert_eq!(s, "-0500");
        }

        #[test]
        fn parses_z_as_zero() {
            let (secs, s) = parse_offset_string("Z").unwrap();
            assert_eq!(secs, 0);
            assert_eq!(s, "Z");
        }

        #[test]
        fn returns_none_for_invalid_offset() {
            assert!(parse_offset_string("invalid").is_none());
            assert!(
                parse_offset_string("+15:00").is_none(),
                "Hour offset should be <= 14"
            );
            assert!(
                parse_offset_string("+02:60").is_none(),
                "Minute offset should be <= 59"
            );
        }
    }

    // --- Tests for `add_subseconds_from_number` ---
    mod add_subseconds_from_number_tests {
        use super::*;

        fn base_dt() -> NaiveDateTime {
            NaiveDate::from_ymd_opt(2024, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
        }

        #[test]
        fn adds_three_digit_subseconds() {
            let dt = add_subseconds_from_number(base_dt(), 123);
            assert_eq!(dt.nanosecond(), 123_000_000);
        }

        #[test]
        fn adds_six_digit_subseconds() {
            let dt = add_subseconds_from_number(base_dt(), 123456);
            assert_eq!(dt.nanosecond(), 123_456_000);
        }

        #[test]
        fn adds_one_digit_subseconds() {
            let dt = add_subseconds_from_number(base_dt(), 7);
            assert_eq!(dt.nanosecond(), 700_000_000);
        }

        #[test]
        fn handles_zero_correctly() {
            let dt = add_subseconds_from_number(base_dt(), 0);
            assert_eq!(dt.nanosecond(), 0);
        }

        #[test]
        fn handles_large_numbers_correctly() {
            // Numbers with >9 digits are truncated to nanosecond precision
            let dt = add_subseconds_from_number(base_dt(), 1234567890);
            assert_eq!(dt.nanosecond(), 234567890);
        }
    }
}
