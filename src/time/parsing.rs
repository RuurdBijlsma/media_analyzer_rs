//! Utility functions for parsing time/date/offset strings into chrono types.

use chrono::{DateTime, FixedOffset, NaiveDateTime, Timelike};
use regex::Regex; // Only if needed directly here, which it is for parse_offset_string

// Make functions `pub` so they can be used by `extraction.rs` and `logic.rs`.

/// Parses a naive datetime string commonly found in EXIF (YYYY:MM:DD HH:MM:SS[.fff]).
/// Returns the NaiveDateTime and a boolean indicating if subseconds were present in the string.
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
    DateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%SZ")
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .or_else(|| {
            if s.ends_with('Z') {
                DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            } else {
                None
            }
        })
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

/// Adds subsecond precision from a separate numeric EXIF field to a NaiveDateTime.
pub fn add_subseconds_from_number(dt: NaiveDateTime, subsec_num: u32) -> NaiveDateTime {
    if subsec_num == 0 {
        return dt;
    }
    let subsec_str = subsec_num.to_string();
    let num_digits = subsec_str.len() as u32;
    let nanos = if num_digits <= 9 {
        subsec_num.saturating_mul(10u32.pow(9u32.saturating_sub(num_digits)))
    } else {
        subsec_num % 1_000_000_000
    };
    dt.with_nanosecond(nanos).unwrap_or(dt)
}
