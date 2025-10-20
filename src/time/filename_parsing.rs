use chrono::{DateTime, NaiveDateTime};
use chrono_tz::Tz;
use regex::Regex;
use std::sync::OnceLock;

static RE_YYYYMMDD_HHMMSS: OnceLock<Regex> = OnceLock::new();
static RE_YYYY_MM_DD_HH_MM_SS: OnceLock<Regex> = OnceLock::new();
static RE_UNIX_MS: OnceLock<Regex> = OnceLock::new();

pub fn parse_datetime_from_filename(
    filename: &str,
    fallback_timezone: Option<Tz>,
) -> Option<NaiveDateTime> {
    // --- Attempt 1: Standard YYYYMMDD_HHMMSS format ---
    // The `get_or_init` method ensures the Regex is compiled exactly once on its first use.
    let re1 = RE_YYYYMMDD_HHMMSS.get_or_init(|| Regex::new(r"(\d{8})_(\d{6})").unwrap());
    if let Some(caps) = re1.captures(filename) {
        if caps.len() == 3 {
            let date_str = &caps[1];
            let time_str = &caps[2];
            let datetime_str = format!("{}{}", date_str, time_str);
            if let Ok(dt) = NaiveDateTime::parse_from_str(&datetime_str, "%Y%m%d%H%M%S") {
                return Some(dt);
            }
        }
    }

    // --- Attempt 2: Hyphenated YYYY-MM-DD_HH-MM-SS format ---
    let re2 = RE_YYYY_MM_DD_HH_MM_SS
        .get_or_init(|| Regex::new(r"(\d{4}-\d{2}-\d{2})_(\d{2}-\d{2}-\d{2})").unwrap());
    if let Some(caps) = re2.captures(filename) {
        if caps.len() == 3 {
            let date_str = &caps[1];
            let time_str = &caps[2];
            let datetime_str = format!("{} {}", date_str, time_str);
            if let Ok(dt) = NaiveDateTime::parse_from_str(&datetime_str, "%Y-%m-%d %H-%M-%S") {
                return Some(dt);
            }
        }
    }

    // --- Attempt 3: Unix Millisecond Timestamp format ---
    let re3 = RE_UNIX_MS.get_or_init(|| Regex::new(r"^(\d{13})\.").unwrap());
    if let Some(caps) = re3.captures(filename) {
        if let Some(timestamp_str) = caps.get(1) {
            if let Ok(ms) = timestamp_str.as_str().parse::<i64>() {
                if let Some(dt) = DateTime::from_timestamp_millis(ms).map(|d| {
                    if let Some(tz) = fallback_timezone {
                        return d.with_timezone(&tz).naive_local();
                    }
                    return d.naive_utc();
                }) {
                    return Some(dt);
                }
            }
        }
    }

    // If no patterns matched, return None
    None
}
