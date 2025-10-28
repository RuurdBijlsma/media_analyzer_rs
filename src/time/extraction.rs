//! Functions for extracting raw time-related string/number values from EXIF JSON.

use super::parsing::{
    add_subseconds_from_number, parse_datetime_offset, parse_datetime_utc_z, parse_naive,
    parse_offset_string,
};
use crate::time::filename_parsing::parse_datetime_from_filename;
use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use chrono_tz::Tz;
use serde_json::Value;

#[derive(Debug)]
/// Intermediate data structure
pub struct ExtractedTimeComponents {
    pub best_local: Option<(NaiveDateTime, String)>, // (DateTime, Source Tag Name)
    pub potential_utc: Option<(DateTime<Utc>, String)>, // (DateTime, Source Tag Name)
    pub potential_explicit_offset: Option<(i32, String, String)>, // (Offset Seconds, Offset String, Source Tag Name)
    pub potential_file_dt: Option<(DateTime<FixedOffset>, String)>, // (DateTime, Source Tag Name)
}

/// Parses a datetime from a filename string.
fn parse_filename_to_naive(
    value: &Value,
    fallback_timezone: Option<Tz>,
) -> Option<(NaiveDateTime, String)> {
    if let Some(filename) = get_string_field(value, "Other", "FileName") {
        let result = parse_datetime_from_filename(filename, fallback_timezone);
        return result.map(|datetime| (datetime, "FileName".to_string()));
    }
    None
}

pub fn extract_time_components(
    exif_info: &Value,
    fallback_timezone: Option<Tz>,
) -> ExtractedTimeComponents {
    let mut potential_utc: Option<(DateTime<Utc>, String)> = None;
    let mut potential_explicit_offset: Option<(i32, String, String)> = None;
    let mut potential_file_dt: Option<(DateTime<FixedOffset>, String)> = None;

    // --- Best Naive Time (DateTimeOriginal, CreateDate, etc.) with Subseconds ---
    let naive_sources_priority = [
        ("Time", "SubSecDateTimeOriginal", true),
        ("Time", "SubSecCreateDate", true),
        ("Time", "SubSecTimeDigitized", true),
        ("Time", "DateTimeOriginal", false),
        ("Time", "CreateDate", false),
        ("Time", "DateTimeDigitized", false),
        ("Time", "SubSecModifyDate", true),
        ("Time", "ModifyDate", false),
    ];

    let mut primary_naive_candidate: Option<(NaiveDateTime, String)> = None;
    let mut found_subsecond_number_source: Option<(String, u32)> = None;

    for (group, field, _is_subsec_field) in naive_sources_priority {
        if primary_naive_candidate.is_none()
            && let Some(dt_str) = get_string_field(exif_info, group, field)
            && let Some((dt, parsed_subsec)) = parse_naive(dt_str)
        {
            let source_name = field.to_string();
            primary_naive_candidate = Some((dt, source_name));
            if parsed_subsec {
                found_subsecond_number_source = Some(("_ParsedFromString_".to_string(), 0));
            }
        }

        if primary_naive_candidate.is_some()
            && found_subsecond_number_source
                .as_ref()
                .is_none_or(|(src, _)| src != "_ParsedFromString_")
        {
            let base_field_name = field.replace("SubSec", "");
            let sub_sec_num_field = format!(
                "SubSecTime{}",
                base_field_name.replace("Date", "").replace("Time", "")
            );

            if let Some(subsec_num) = get_number_field(exif_info, group, &sub_sec_num_field)
                && primary_naive_candidate
                    .as_ref()
                    .is_some_and(|(_, src)| *src == base_field_name || *src == field)
            {
                found_subsecond_number_source = Some((sub_sec_num_field, subsec_num));
            }

            let simpler_sub_sec_field =
                format!("SubSecond{}", base_field_name.replace("DateTime", ""));
            if found_subsecond_number_source.is_none()
                && let Some(subsec_num) = get_number_field(exif_info, group, &simpler_sub_sec_field)
                && primary_naive_candidate
                    .as_ref()
                    .is_some_and(|(_, src)| *src == base_field_name || *src == field)
            {
                found_subsecond_number_source = Some((simpler_sub_sec_field, subsec_num));
            }
        }

        if primary_naive_candidate.is_some() && found_subsecond_number_source.is_some() {
            break;
        }
        if primary_naive_candidate.is_some() && field == naive_sources_priority.last().unwrap().1 {
            break;
        }
    }

    if let (Some((local_dt, source_name)), Some((subsec_source, subsec_num))) = (
        primary_naive_candidate.as_mut(),
        found_subsecond_number_source.as_ref(),
    ) {
        if subsec_source == "_ParsedFromString_" {
            *source_name = format!("{source_name}: Parsed SubSeconds");
        } else {
            *local_dt = add_subseconds_from_number(*local_dt, *subsec_num);
            *source_name = format!("{source_name} + {subsec_source}");
        }
    }
    let best_local_from_exif = primary_naive_candidate;

    // --- Potential UTC Time ---
    if let Some(gps_dt_str) = get_string_field(exif_info, "Time", "GPSDateTime")
        && let Some(dt_utc) = parse_datetime_utc_z(gps_dt_str)
    {
        potential_utc = Some((dt_utc, "GPSDateTime".to_string()));
    }
    if potential_utc.is_none()
        && let (Some(date_str), Some(time_str)) = (
            get_string_field(exif_info, "Time", "GPSDateStamp"),
            get_string_field(exif_info, "Time", "GPSTimeStamp"),
        )
    {
        let combined_str = format!("{date_str} {time_str}Z");
        if let Some(dt_utc) = parse_datetime_utc_z(&combined_str) {
            potential_utc = Some((dt_utc, "GPSDateStamp/GPSTimeStamp".to_string()));
        }
    }

    // --- Potential Explicit Offset ---
    let offset_sources_priority = [
        ("Time", "OffsetTimeOriginal"),
        ("Time", "OffsetTimeDigitized"),
        ("Time", "OffsetTime"),
    ];
    for (group, field) in offset_sources_priority {
        if let Some(offset_str) = get_string_field(exif_info, group, field)
            && let Some((secs, parsed_str)) = parse_offset_string(offset_str)
        {
            potential_explicit_offset = Some((secs, parsed_str, field.to_string()));
            break;
        }
    }

    // --- Potential File Time ---
    let file_time_sources_priority = [
        ("Time", "FileModifyDate"),
        ("Time", "FileCreateDate"),
        ("Time", "FileAccessDate"),
    ];
    for (group, field) in file_time_sources_priority {
        if let Some(dt_str) = get_string_field(exif_info, group, field)
            && let Some(dt) = parse_datetime_offset(dt_str)
        {
            potential_file_dt = Some((dt, field.to_string()));
            break;
        }
    }

    // The filename is now the final fallback for best_local within the extraction step.
    let best_local =
        best_local_from_exif.or_else(|| parse_filename_to_naive(exif_info, fallback_timezone));

    ExtractedTimeComponents {
        best_local,
        potential_utc,
        potential_explicit_offset,
        potential_file_dt,
    }
}

/// Safely extracts a string field from nested JSON Value.
pub fn get_string_field<'a>(value: &'a Value, group: &str, field: &str) -> Option<&'a str> {
    value.get(group)?.get(field)?.as_str()
}

/// Safely extracts a number field (as u32) from nested JSON Value.
fn get_number_field(value: &Value, group: &str, field: &str) -> Option<u32> {
    value
        .get(group)?
        .get(field)?
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use serde_json::json;

    #[test]
    fn test_extracts_nothing_from_empty_json() {
        let exif = json!({});
        let components = extract_time_components(&exif, None);

        assert!(components.best_local.is_none());
        assert!(components.potential_utc.is_none());
        assert!(components.potential_explicit_offset.is_none());
        assert!(components.potential_file_dt.is_none());
    }

    #[test]
    fn test_best_local_falls_back_to_filename() {
        // This JSON has no standard EXIF date tags, so the function must parse the filename.
        let exif = json!({
            "Other": {
                "FileName": "IMG_20240101_123000.jpg"
            }
        });
        let tz = "Europe/Amsterdam".parse::<Tz>().unwrap();
        let components = extract_time_components(&exif, Some(tz));

        assert!(components.best_local.is_some());
        let (local_dt, source) = components.best_local.unwrap();
        assert_eq!(source, "FileName");
        assert_eq!(
            local_dt,
            NaiveDate::from_ymd_opt(2024, 1, 1)
                .unwrap()
                .and_hms_opt(12, 30, 0)
                .unwrap()
        );
    }

    #[test]
    fn test_best_local_falls_back_to_filename_w_fallback_tz() {
        // This JSON has no standard EXIF date tags, so the function must parse the epoch time filename.
        let exif = json!({
            "Other": {
                "FileName": "1597948682906.jpg"
            }
        });
        let tz = "Europe/Amsterdam".parse::<Tz>().unwrap();
        let components = extract_time_components(&exif, Some(tz));

        assert!(components.best_local.is_some());
        let (local_dt, source) = components.best_local.unwrap();
        assert_eq!(source, "FileName");
        assert_eq!(
            local_dt,
            NaiveDate::from_ymd_opt(2020, 8, 20)
                .unwrap()
                .and_hms_milli_opt(20, 38, 2, 906)
                .unwrap()
        );
    }

    #[test]
    fn test_exif_date_is_preferred_over_filename() {
        // This JSON has both a valid EXIF tag and a filename. The EXIF tag should win.
        let exif = json!({
            "Time": {
                "DateTimeOriginal": "2025:02:02 11:11:11"
            },
            "Other": {
                "FileName": "IMG_20240101_123000.jpg"
            }
        });
        let components = extract_time_components(&exif, None);

        assert!(components.best_local.is_some());
        let (local_dt, source) = components.best_local.unwrap();
        assert_eq!(source, "DateTimeOriginal"); // Verifies EXIF was preferred
        assert_eq!(
            local_dt,
            NaiveDate::from_ymd_opt(2025, 2, 2)
                .unwrap()
                .and_hms_opt(11, 11, 11)
                .unwrap()
        );
    }

    #[test]
    fn test_naive_time_priority_logic() {
        // CreateDate is lower priority than DateTimeOriginal
        let exif = json!({
            "Time": {
                "CreateDate": "2023:01:01 10:00:00",
                "DateTimeOriginal": "2024:02:02 12:34:56"
            }
        });

        let components = extract_time_components(&exif, None);
        assert!(components.best_local.is_some());

        let (local_dt, source) = components.best_local.unwrap();
        assert_eq!(source, "DateTimeOriginal");
        assert_eq!(
            local_dt,
            NaiveDate::from_ymd_opt(2024, 2, 2)
                .unwrap()
                .and_hms_opt(12, 34, 56)
                .unwrap()
        );
    }

    #[test]
    fn test_naive_time_with_parsed_subseconds() {
        // Subseconds are part of the string itself
        let exif = json!({
            "Time": {
                "SubSecDateTimeOriginal": "2024:03:03 11:22:33.123"
            }
        });

        let components = extract_time_components(&exif, None);
        let (local_dt, source) = components.best_local.unwrap();

        assert_eq!(source, "SubSecDateTimeOriginal: Parsed SubSeconds");
        assert_eq!(
            local_dt,
            NaiveDate::from_ymd_opt(2024, 3, 3)
                .unwrap()
                .and_hms_micro_opt(11, 22, 33, 123_000)
                .unwrap()
        );
    }

    #[test]
    fn test_naive_time_with_separate_subsecond_field() {
        // Subseconds are in a separate numeric tag
        let exif = json!({
            "Time": {
                "DateTimeOriginal": "2024:04:04 14:15:16",
                "SubSecTimeOriginal": 456
            }
        });

        let components = extract_time_components(&exif, None);
        let (local_dt, source) = components.best_local.unwrap();

        // Check that the source name was correctly combined
        assert_eq!(source, "DateTimeOriginal + SubSecTimeOriginal");
        assert_eq!(
            local_dt,
            NaiveDate::from_ymd_opt(2024, 4, 4)
                .unwrap()
                .and_hms_micro_opt(14, 15, 16, 456_000)
                .unwrap()
        );
    }

    #[test]
    fn test_utc_time_extraction() {
        // Primary case: GPSDateTime
        let exif_gps_dt = json!({
            "Time": { "GPSDateTime": "2024:05:05 10:00:00Z" }
        });
        let components_1 = extract_time_components(&exif_gps_dt, None);
        let (utc_dt_1, source_1) = components_1.potential_utc.unwrap();
        assert_eq!(source_1, "GPSDateTime");
        assert_eq!(utc_dt_1.to_rfc3339(), "2024-05-05T10:00:00+00:00");

        // Fallback case: GPSDateStamp + GPSTimeStamp
        let exif_gps_stamps = json!({
            "Time": {
                "GPSDateStamp": "2024:06:06",
                "GPSTimeStamp": "11:22:33"
            }
        });
        let components_2 = extract_time_components(&exif_gps_stamps, None);
        let (utc_dt_2, source_2) = components_2.potential_utc.unwrap();
        assert_eq!(source_2, "GPSDateStamp/GPSTimeStamp");
        assert_eq!(utc_dt_2.to_rfc3339(), "2024-06-06T11:22:33+00:00");
    }

    #[test]
    fn test_offset_and_file_time_priority() {
        let exif = json!({
            "Time": {
                // Offset: Original is highest priority
                "OffsetTime": "+05:00",
                "OffsetTimeOriginal": "-04:00",

                // File Time: Modify is highest priority
                "FileAccessDate": "2023:01:01 10:00:00+01:00",
                "FileModifyDate": "2024:07:07 15:00:00-07:00"
            }
        });

        let components = extract_time_components(&exif, None);

        // Verify Offset Time
        assert!(components.potential_explicit_offset.is_some());
        let (secs, parsed_str, source) = components.potential_explicit_offset.unwrap();
        assert_eq!(source, "OffsetTimeOriginal");
        assert_eq!(parsed_str, "-04:00");
        assert_eq!(secs, -4 * 3600);

        // Verify File Time
        assert!(components.potential_file_dt.is_some());
        let (file_dt, file_source) = components.potential_file_dt.unwrap();
        assert_eq!(file_source, "FileModifyDate");
        assert_eq!(file_dt.to_rfc3339(), "2024-07-07T15:00:00-07:00");
    }
}
