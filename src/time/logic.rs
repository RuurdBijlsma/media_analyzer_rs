//! Core logic for determining the best time representation based on extracted components.

use super::error::TimeError;
use super::extraction::{ExtractedTimeComponents, extract_time_components, get_string_field};
use crate::GpsInfo;
use crate::time::structs::{
    CONFIDENCE_HIGH, CONFIDENCE_LOW, CONFIDENCE_MEDIUM, SourceDetails, TimeInfo, TimeZoneInfo,
};
use chrono::{
    DateTime, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Utc,
};
use chrono_tz::Tz;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use std::str::FromStr;
use tzf_rs::DefaultFinder;

// --- Constants specific to the logic ---
const MAX_NAIVE_GPS_DIFF_SECONDS: i64 = 10;

// --- Global Timezone Finder ---
static FINDER: Lazy<DefaultFinder> = Lazy::new(DefaultFinder::new);

/// Main entry point function - Extracts and processes time info.
///
/// Returns a `Result` which is `Ok(TimeInfo)` on success or an `Err(TimeError)`
/// if no usable time information could be extracted from any source.
pub fn get_time_info(exif_info: &Value, gps_info: Option<&GpsInfo>) -> Result<TimeInfo, TimeError> {
    let components = extract_time_components(exif_info);
    let time_result = apply_priority_logic(components, gps_info, exif_info);

    // If apply_priority_logic returns None, map it to our custom Extraction error.
    // This replaces the previous `unwrap_or_else` block that created a dummy/error TimeInfo.
    time_result.ok_or(TimeError::Extraction)
}

/// Applies the priority logic to extracted components and constructs the final TimeInfo.
/// Kept private to this module, called by `get_time_info`.
fn apply_priority_logic(
    components: ExtractedTimeComponents,
    gps_info: Option<&GpsInfo>,
    exif_info: &Value, // Needed for filename fallback
) -> Option<TimeInfo> {
    let ExtractedTimeComponents {
        best_naive,
        potential_utc,
        potential_explicit_offset,
        potential_file_dt,
    } = components;

    // --- Priority 1: Confirmed UTC ---
    // Requires: best_naive, potential_utc, AND gps_info to determine local offset
    if let (
        Some((naive_dt, naive_source)), // Local naive time from camera
        Some((gps_utc_dt, utc_source)), // UTC time from GPS
        Some(gps),                      // GPS coordinates
    ) = (&best_naive, &potential_utc, gps_info)
    {
        // Find the IANA timezone name from GPS coordinates
        let tz_name = FINDER.get_tz_name(gps.longitude, gps.latitude);
        if let Ok(tz) = Tz::from_str(tz_name) {
            // Attempt to interpret the camera's naive time in the location's timezone.
            // Handle ambiguity (e.g., DST) by preferring the earlier time (common).
            if let LocalResult::Single(zoned_dt) | LocalResult::Ambiguous(zoned_dt, _) =
                tz.from_local_datetime(naive_dt)
            {
                // Convert the interpreted local time to UTC
                let calculated_utc_from_naive = zoned_dt.with_timezone(&Utc);

                // Compare the UTC calculated from naive+GPS_location with the direct GPS_UTC time
                let diff = gps_utc_dt.signed_duration_since(calculated_utc_from_naive);

                // Check if the difference is within the tolerance
                if diff.num_seconds().abs() <= MAX_NAIVE_GPS_DIFF_SECONDS {
                    // SUCCESS: GPS UTC confirms the local naive time + location.
                    let offset_secs = zoned_dt.offset().fix().local_minus_utc();
                    let tz_info = TimeZoneInfo {
                        name: tz.name().to_string(), // Use the IANA name found
                        offset_seconds: offset_secs,
                        source: format!(
                            "{} confirmed by {} @ GPS location",
                            utc_source, naive_source
                        ),
                    };
                    return Some(TimeInfo {
                        datetime_utc: Some(*gps_utc_dt), // Trust the direct GPS UTC time
                        datetime_naive: *naive_dt,       // Keep the original naive time
                        timezone: Some(tz_info),
                        source_details: SourceDetails {
                            time_source: naive_source.clone(),
                            confidence: CONFIDENCE_HIGH.to_string(),
                        },
                    });
                }
            }
        }
    }

    // --- Priority 2: Zoned Time (Naive + GPS -> IANA TZ) ---
    if let Some((naive_dt, ref naive_source)) = best_naive {
        if let Some(gps) = gps_info {
            let tz_name = FINDER.get_tz_name(gps.longitude, gps.latitude);
            if let Ok(tz) = Tz::from_str(tz_name)
                && let LocalResult::Single(zoned_dt) | LocalResult::Ambiguous(zoned_dt, _) =
                    tz.from_local_datetime(&naive_dt)
            {
                let utc_dt = zoned_dt.with_timezone(&Utc);
                let offset_secs = zoned_dt.offset().fix().local_minus_utc();
                return Some(TimeInfo {
                    datetime_utc: Some(utc_dt),
                    datetime_naive: naive_dt,
                    timezone: Some(TimeZoneInfo {
                        name: tz.name().to_string(),
                        offset_seconds: offset_secs,
                        source: "IANA from GPS".to_string(),
                    }),
                    source_details: SourceDetails {
                        time_source: naive_source.clone(),
                        confidence: CONFIDENCE_HIGH.to_string(),
                    },
                });
            }
        }

        // --- Priority 3: Fixed Offset Time (Naive + Explicit Offset) ---
        if let Some((offset_secs, ref offset_str, ref offset_source)) = potential_explicit_offset
            && let Some(offset) = FixedOffset::east_opt(offset_secs)
            && let LocalResult::Single(dt_with_offset) | LocalResult::Ambiguous(dt_with_offset, _) =
                offset.from_local_datetime(&naive_dt)
        {
            let utc_dt = dt_with_offset.with_timezone(&Utc);
            return Some(TimeInfo {
                datetime_utc: Some(utc_dt),
                datetime_naive: naive_dt,
                timezone: Some(TimeZoneInfo {
                    name: offset_str.clone(),
                    offset_seconds: offset_secs,
                    source: offset_source.clone(),
                }),
                source_details: SourceDetails {
                    time_source: naive_source.clone(),
                    confidence: CONFIDENCE_HIGH.to_string(),
                },
            });
        }
    }

    // --- Priority 4: Accurate UTC ---
    if let Some((utc_dt, ref utc_source)) = potential_utc {
        return Some(TimeInfo {
            datetime_utc: Some(utc_dt),
            datetime_naive: utc_dt.naive_utc(),
            timezone: Some(TimeZoneInfo {
                name: "UTC".to_string(),
                offset_seconds: 0,
                source: utc_source.clone(),
            }),
            source_details: SourceDetails {
                time_source: utc_source.clone(),
                confidence: CONFIDENCE_HIGH.to_string(),
            },
        });
    }

    // --- Priority 5: Naive With Guessed Offset ---
    if let Some((naive_dt, ref naive_source)) = best_naive {
        return if let Some((file_dt, ref file_source)) = potential_file_dt {
            let guessed_offset = file_dt.offset().fix();
            let offset_source_str = format!("Guessed from {}", file_source);
            let mut iso_utc: Option<DateTime<Utc>> = None;

            if let LocalResult::Single(guessed_dt_offset)
            | LocalResult::Ambiguous(guessed_dt_offset, _) =
                guessed_offset.from_local_datetime(&naive_dt)
            {
                iso_utc = Some(guessed_dt_offset.with_timezone(&Utc));
            }

            Some(TimeInfo {
                datetime_utc: iso_utc,
                datetime_naive: naive_dt,
                timezone: Some(TimeZoneInfo {
                    name: guessed_offset.to_string(),
                    offset_seconds: guessed_offset.local_minus_utc(),
                    source: offset_source_str,
                }),
                source_details: SourceDetails {
                    time_source: naive_source.clone(),
                    confidence: CONFIDENCE_MEDIUM.to_string(),
                },
            })
        } else {
            // --- Priority 6: Naive Only (from EXIF) ---
            Some(TimeInfo {
                datetime_utc: None,
                datetime_naive: naive_dt,
                timezone: None,
                source_details: SourceDetails {
                    time_source: naive_source.clone(),
                    confidence: CONFIDENCE_LOW.to_string(),
                },
            })
        };
    }

    // --- Priority 7: Filename Parsing ---
    if best_naive.is_none() {
        // Use get_string_field from the extraction module
        if let Some(filename) = get_string_field(exif_info, "Other", "FileName") {
            let re = Regex::new(
                r"\b(\d{4})[-_]?(\d{2})[-_]?(\d{2})[-_ ]?(\d{2})[:_]?(\d{2})[:_]?(\d{2})\b",
            )
            .ok();
            if let Some(re) = re
                && let Some(caps) = re.captures(filename)
                && let (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(min), Ok(sec)) = (
                    caps.get(1)
                        .map_or(Err(()), |m| m.as_str().parse::<i32>().map_err(|_| ())),
                    caps.get(2)
                        .map_or(Err(()), |m| m.as_str().parse::<u32>().map_err(|_| ())),
                    caps.get(3)
                        .map_or(Err(()), |m| m.as_str().parse::<u32>().map_err(|_| ())),
                    caps.get(4)
                        .map_or(Err(()), |m| m.as_str().parse::<u32>().map_err(|_| ())),
                    caps.get(5)
                        .map_or(Err(()), |m| m.as_str().parse::<u32>().map_err(|_| ())),
                    caps.get(6)
                        .map_or(Err(()), |m| m.as_str().parse::<u32>().map_err(|_| ())),
                )
                && let (Some(date), Some(time)) = (
                    NaiveDate::from_ymd_opt(year, month, day),
                    NaiveTime::from_hms_opt(hour, min, sec),
                )
            {
                return Some(TimeInfo {
                    datetime_utc: None,
                    datetime_naive: NaiveDateTime::new(date, time),
                    timezone: None,
                    source_details: SourceDetails {
                        time_source: "FileName".to_string(),
                        confidence: CONFIDENCE_LOW.to_string(),
                    },
                });
            }
        }
    }

    // --- Priority 8: File Metadata Time (Last Resort) ---
    // This is reached if all other higher-priority methods failed.
    // We use potential_file_dt which might have been extracted earlier.
    // Note: We didn't use this for the 'guessed offset' in P5 because P5 required a 'best_naive' time first.
    if let Some((file_dt, ref file_source)) = potential_file_dt {
        // We have a file time (like FileModifyDate) with an offset.
        let utc_dt = file_dt.with_timezone(&Utc);
        let offset = file_dt.offset().fix(); // Get the original offset

        return Some(TimeInfo {
            datetime_utc: Some(utc_dt),
            // The 'naive' time is the file's time as it was recorded (local perspective)
            datetime_naive: file_dt.naive_local(),
            timezone: Some(TimeZoneInfo {
                name: offset.to_string(), // e.g., "+02:00"
                offset_seconds: offset.local_minus_utc(),
                source: file_source.clone(), // e.g., "FileModifyDate"
            }),
            source_details: SourceDetails {
                // The primary source *is* the file metadata tag itself in this case
                time_source: file_source.clone(),
                // Confidence is LOW because file times are unreliable for capture time
                confidence: CONFIDENCE_LOW.to_string(),
            },
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MediaAnalyzerError;
    use crate::features::gps::{GpsInfo, LocationName};
    use chrono::{FixedOffset, NaiveDate};
    use exiftool::ExifTool;
    use std::path::Path;

    // --- UNIT TESTS (Mocked data, focused on priority logic) ---

    #[test]
    fn test_priority_6_naive_only() -> Result<(), MediaAnalyzerError> {
        // Test the simplest case: only a naive datetime is available.
        let components = ExtractedTimeComponents {
            best_naive: Some((
                NaiveDate::from_ymd_opt(2023, 1, 1)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap(),
                "DateTimeOriginal".to_string(),
            )),
            potential_utc: None,
            potential_explicit_offset: None,
            potential_file_dt: None,
        };
        let time_info = apply_priority_logic(components, None, &serde_json::Value::Null).unwrap();
        assert_eq!(time_info.source_details.confidence, CONFIDENCE_LOW);
        assert_eq!(time_info.source_details.time_source, "DateTimeOriginal");

        Ok(())
    }

    #[test]
    fn test_priority_5_naive_with_guessed_offset() -> Result<(), MediaAnalyzerError> {
        // Test that it correctly uses the file modification date to guess an offset.
        let components = ExtractedTimeComponents {
            best_naive: Some((
                NaiveDate::from_ymd_opt(2023, 1, 1)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap(),
                "CreateDate".to_string(),
            )),
            potential_utc: None,
            potential_explicit_offset: None,
            potential_file_dt: Some((
                FixedOffset::east_opt(2 * 3600)
                    .unwrap()
                    .with_ymd_and_hms(2023, 1, 1, 14, 0, 0)
                    .unwrap(),
                "FileModifyDate".to_string(),
            )),
        };
        let time_info = apply_priority_logic(components, None, &serde_json::Value::Null).unwrap();
        assert_eq!(time_info.source_details.confidence, CONFIDENCE_MEDIUM);
        let tz = time_info.timezone.unwrap();
        assert_eq!(tz.offset_seconds, 2 * 3600);
        assert_eq!(tz.source, "Guessed from FileModifyDate");

        Ok(())
    }

    #[test]
    fn test_priority_3_fixed_offset() -> Result<(), MediaAnalyzerError> {
        // Test that an explicit offset tag is preferred over a guessed offset.
        let components = ExtractedTimeComponents {
            best_naive: Some((
                NaiveDate::from_ymd_opt(2024, 1, 1)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                "DateTimeOriginal".to_string(),
            )),
            potential_utc: None,
            potential_explicit_offset: Some((
                -5 * 3600,
                "-05:00".to_string(),
                "OffsetTime".to_string(),
            )),
            potential_file_dt: Some((
                // This lower-priority data should be ignored
                FixedOffset::east_opt(2 * 3600)
                    .unwrap()
                    .with_ymd_and_hms(2024, 1, 1, 12, 0, 0)
                    .unwrap(),
                "FileModifyDate".to_string(),
            )),
        };
        let time_info = apply_priority_logic(components, None, &serde_json::Value::Null).unwrap();
        assert_eq!(time_info.source_details.confidence, CONFIDENCE_HIGH);
        assert_eq!(time_info.timezone.unwrap().source, "OffsetTime");

        Ok(())
    }

    #[test]
    fn test_priority_2_zoned_time_with_gps() -> Result<(), MediaAnalyzerError> {
        // Test that GPS location correctly determines the IANA timezone. (Amsterdam case)
        let amsterdam_gps = GpsInfo {
            latitude: 52.379189,
            longitude: 4.899431,
            altitude: None,
            location: LocationName {
                latitude: 52.37,
                longitude: 4.89,
                name: "Amsterdam".to_string(),
                admin1: "".to_string(),
                admin2: "".to_string(),
                country_code: "NL".to_string(),
                country_name: None,
            },
            image_direction: None,
            image_direction_ref: None,
        };
        let components = ExtractedTimeComponents {
            best_naive: Some((
                NaiveDate::from_ymd_opt(2024, 7, 1)
                    .unwrap()
                    .and_hms_opt(15, 0, 0)
                    .unwrap(), // 3 PM in summer
                "DateTimeOriginal".to_string(),
            )),
            potential_utc: None,
            potential_explicit_offset: None,
            potential_file_dt: None,
        };
        let time_info =
            apply_priority_logic(components, Some(&amsterdam_gps), &serde_json::Value::Null)
                .unwrap();
        assert_eq!(time_info.source_details.confidence, CONFIDENCE_HIGH);
        let tz = time_info.timezone.unwrap();
        assert_eq!(tz.name, "Europe/Amsterdam");
        assert_eq!(
            tz.offset_seconds,
            2 * 3600,
            "Amsterdam is UTC+2 in summer (DST)"
        );

        Ok(())
    }

    // --- INTEGRATION TESTS (Using real asset files with the correct `-g2` flag) ---

    /// Helper that runs exiftool with the `-g2` flag, which is specifically
    /// required by the time extraction logic.
    fn get_g2_exif_for_asset(relative_path: &str) -> Result<Value, MediaAnalyzerError> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join(relative_path);
        let mut et = ExifTool::new()?;
        // The `-g2` flag provides the grouped JSON structure this module expects.
        Ok(et.json(&path, &["-g2"])?)
    }

    #[test]
    fn test_real_life_photo() -> Result<(), MediaAnalyzerError> {
        // This burst photo has a timestamp in its filename but lacks high-quality EXIF date tags.
        let exif_data = get_g2_exif_for_asset("burst/20150813_160421_Burst01.jpg")?;
        let time_info = get_time_info(&exif_data, None)?;

        assert_eq!(time_info.source_details.confidence, CONFIDENCE_HIGH);
        assert_eq!(time_info.source_details.time_source, "GPSDateTime");
        assert!(time_info.datetime_utc.is_some());
        assert_eq!(
            time_info.datetime_naive,
            NaiveDate::from_ymd_opt(2015, 8, 13)
                .unwrap()
                .and_hms_opt(14, 5, 18)
                .unwrap()
        );

        Ok(())
    }

    #[test]
    fn test_real_file_filemodifydate_priority_8() -> Result<(), MediaAnalyzerError> {
        // A text file has no EXIF or filename time, so it must fall back to the filesystem's modify date.
        let exif_data = get_g2_exif_for_asset("text_file.txt")?;
        let time_info = get_time_info(&exif_data, None)?;

        assert_eq!(time_info.source_details.confidence, CONFIDENCE_LOW);
        // The source is just the tag name, as extracted.
        assert_eq!(time_info.source_details.time_source, "FileModifyDate");
        assert!(
            time_info.datetime_utc.is_some(),
            "FileModifyDate includes an offset, so UTC can be calculated"
        );

        Ok(())
    }
}
