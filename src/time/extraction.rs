//! Functions for extracting raw time-related string/number values from EXIF JSON.

use super::parsing::{
    add_subseconds_from_number, parse_datetime_offset, parse_datetime_utc_z, parse_naive,
    parse_offset_string,
};
use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use serde_json::Value;

#[derive(Debug)]
pub struct ExtractedTimeComponents {
    pub best_naive: Option<(NaiveDateTime, String)>, // (DateTime, Source Tag Name)
    pub potential_utc: Option<(DateTime<Utc>, String)>, // (DateTime, Source Tag Name)
    pub potential_explicit_offset: Option<(i32, String, String)>, // (Offset Seconds, Offset String, Source Tag Name)
    pub potential_file_dt: Option<(DateTime<FixedOffset>, String)>, // (DateTime, Source Tag Name)
}

pub fn extract_time_components(exif_info: &Value) -> ExtractedTimeComponents {
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
        if primary_naive_candidate.is_none() {
            if let Some(dt_str) = get_string_field(exif_info, group, field) {
                if let Some((dt, parsed_subsec)) = parse_naive(dt_str) {
                    let source_name = field.to_string();
                    primary_naive_candidate = Some((dt, source_name));
                    if parsed_subsec {
                        found_subsecond_number_source = Some(("_ParsedFromString_".to_string(), 0));
                    }
                }
            }
        }

        if primary_naive_candidate.is_some()
            && !found_subsecond_number_source
            .as_ref()
            .is_some_and(|(src, _)| src == "_ParsedFromString_")
        {
            let base_field_name = field.replace("SubSec", "");
            let sub_sec_num_field = format!(
                "SubSecTime{}",
                base_field_name.replace("Date", "").replace("Time", "")
            );

            if let Some(subsec_num) = get_number_field(exif_info, group, &sub_sec_num_field) {
                if primary_naive_candidate
                    .as_ref()
                    .is_some_and(|(_, src)| *src == base_field_name || *src == field)
                {
                    found_subsecond_number_source = Some((sub_sec_num_field, subsec_num));
                }
            }

            let simpler_sub_sec_field = format!("SubSecond{}", base_field_name.replace("DateTime", ""));
            if found_subsecond_number_source.is_none() {
                if let Some(subsec_num) = get_number_field(exif_info, group, &simpler_sub_sec_field) {
                    if primary_naive_candidate
                        .as_ref()
                        .is_some_and(|(_, src)| *src == base_field_name || *src == field)
                    {
                        found_subsecond_number_source = Some((simpler_sub_sec_field, subsec_num));
                    }
                }
            }
        }

        if primary_naive_candidate.is_some() && found_subsecond_number_source.is_some() {
            break;
        }
        if primary_naive_candidate.is_some() && field == naive_sources_priority.last().unwrap().1 {
            break;
        }
    }

    if let (Some((naive_dt, source_name)), Some((subsec_source, subsec_num))) = (
        primary_naive_candidate.as_mut(),
        found_subsecond_number_source.as_ref(),
    ) {
        if subsec_source != "_ParsedFromString_" {
            *naive_dt = add_subseconds_from_number(*naive_dt, *subsec_num);
            *source_name = format!("{} + {}", source_name, subsec_source);
        } else {
            *source_name = format!("{}: Parsed SubSeconds", source_name);
        }
    }
    let best_naive = primary_naive_candidate;

    // --- Potential UTC Time ---
    if let Some(gps_dt_str) = get_string_field(exif_info, "Time", "GPSDateTime") {
        if let Some(dt_utc) = parse_datetime_utc_z(gps_dt_str) {
            potential_utc = Some((dt_utc, "GPSDateTime".to_string()));
        }
    }
    if potential_utc.is_none() {
        if let (Some(date_str), Some(time_str)) = (
            get_string_field(exif_info, "Time", "GPSDateStamp"),
            get_string_field(exif_info, "Time", "GPSTimeStamp"),
        ) {
            let combined_str = format!("{} {}Z", date_str, time_str);
            if let Some(dt_utc) = parse_datetime_utc_z(&combined_str) {
                potential_utc = Some((dt_utc, "GPSDateStamp/GPSTimeStamp".to_string()));
            }
        }
    }

    // --- Potential Explicit Offset ---
    let offset_sources_priority = [
        ("Time", "OffsetTimeOriginal"),
        ("Time", "OffsetTimeDigitized"),
        ("Time", "OffsetTime"),
    ];
    for (group, field) in offset_sources_priority {
        if let Some(offset_str) = get_string_field(exif_info, group, field) {
            if let Some((secs, parsed_str)) = parse_offset_string(offset_str) {
                potential_explicit_offset = Some((secs, parsed_str, field.to_string()));
                break;
            }
        }
    }

    // --- Potential File Time ---
    let file_time_sources_priority = [
        ("Time", "FileModifyDate"),
        ("Time", "FileCreateDate"),
        ("Time", "FileAccessDate"),
    ];
    for (group, field) in file_time_sources_priority {
        if let Some(dt_str) = get_string_field(exif_info, group, field) {
            if let Some(dt) = parse_datetime_offset(dt_str) {
                potential_file_dt = Some((dt, field.to_string()));
                break;
            }
        }
    }

    ExtractedTimeComponents {
        best_naive,
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