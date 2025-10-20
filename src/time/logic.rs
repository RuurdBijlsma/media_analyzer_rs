//! Core logic for determining the best time representation based on extracted components.

use super::error::TimeError;
use super::extraction::{extract_time_components, ExtractedTimeComponents};
use crate::time::structs::{
    SourceDetails, TimeInfo, TimeZoneInfo, CONFIDENCE_HIGH, CONFIDENCE_LOW, CONFIDENCE_MEDIUM,
};
use crate::GpsInfo;
use chrono::{
    FixedOffset, LocalResult, Offset, TimeZone, Utc,
};
use chrono_tz::Tz;
use serde_json::Value;
use std::str::FromStr;
use tzf_rs::DefaultFinder;

// --- Constants specific to the logic ---
const MAX_NAIVE_GPS_DIFF_SECONDS: i64 = 10;

// --- Global Timezone Finder ---
static FINDER: std::sync::LazyLock<DefaultFinder> = std::sync::LazyLock::new(DefaultFinder::new);

pub fn get_time_info(exif_info: &Value, gps_info: Option<&GpsInfo>, fallback_timezone: Option<Tz>) -> Result<TimeInfo, TimeError> {
    let components = extract_time_components(exif_info, fallback_timezone);
    let time_result = apply_priority_logic(components, gps_info);
    time_result.ok_or(TimeError::Extraction)
}

/// Applies the priority logic to extracted components and constructs the final `TimeInfo`.
fn apply_priority_logic(
    components: ExtractedTimeComponents,
    gps_info: Option<&GpsInfo>,
) -> Option<TimeInfo> { // MODIFIED: exif_info parameter removed
    let ExtractedTimeComponents {
        best_local,
        potential_utc,
        potential_explicit_offset,
        potential_file_dt,
    } = components;

    // --- Priority 1: Confirmed UTC (Highest confidence) ---
    if let (
        Some((naive_dt, naive_source)),
        Some((gps_utc_dt, utc_source)),
        Some(gps),
    ) = (&best_local, &potential_utc, gps_info) {
        if let Ok(tz) = Tz::from_str(FINDER.get_tz_name(gps.longitude, gps.latitude)) {
            if let LocalResult::Single(zoned_dt) | LocalResult::Ambiguous(zoned_dt, _) =
                tz.from_local_datetime(naive_dt)
            {
                let calculated_utc_from_naive = zoned_dt.with_timezone(&Utc);
                let diff = gps_utc_dt.signed_duration_since(calculated_utc_from_naive);

                if diff.num_seconds().abs() <= MAX_NAIVE_GPS_DIFF_SECONDS {
                    let offset_secs = zoned_dt.offset().fix().local_minus_utc();
                    let tz_info = TimeZoneInfo {
                        name: tz.name().to_string(),
                        offset_seconds: offset_secs,
                        source: format!("{utc_source} confirmed by {naive_source} @ GPS location"),
                    };
                    return Some(TimeInfo {
                        datetime_utc: Some(*gps_utc_dt),
                        datetime_local: *naive_dt,
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

    // --- Main Logic Path: We have a candidate for local time, now find its context. ---
    if let Some((naive_dt, naive_source)) = best_local {
        // --- Priority 2: Zoned Time (Naive + GPS Location) ---
        if let Some(gps) = gps_info {
            if let Ok(tz) = Tz::from_str(FINDER.get_tz_name(gps.longitude, gps.latitude)) {
                if let LocalResult::Single(zoned_dt) | LocalResult::Ambiguous(zoned_dt, _) =
                    tz.from_local_datetime(&naive_dt)
                {
                    return Some(TimeInfo {
                        datetime_utc: Some(zoned_dt.with_timezone(&Utc)),
                        datetime_local: naive_dt,
                        timezone: Some(TimeZoneInfo {
                            name: tz.name().to_string(),
                            offset_seconds: zoned_dt.offset().fix().local_minus_utc(),
                            source: "IANA from GPS".to_string(),
                        }),
                        source_details: SourceDetails {
                            time_source: naive_source,
                            confidence: CONFIDENCE_HIGH.to_string(),
                        },
                    });
                }
            }
        }

        // --- Priority 3: Fixed Offset Time (Naive + Explicit Offset Tag) ---
        if let Some((offset_secs, offset_str, offset_source)) = potential_explicit_offset {
            if let Some(offset) = FixedOffset::east_opt(offset_secs) {
                if let LocalResult::Single(dt_with_offset) | LocalResult::Ambiguous(dt_with_offset, _) =
                    offset.from_local_datetime(&naive_dt)
                {
                    return Some(TimeInfo {
                        datetime_utc: Some(dt_with_offset.with_timezone(&Utc)),
                        datetime_local: naive_dt,
                        timezone: Some(TimeZoneInfo {
                            name: offset_str,
                            offset_seconds: offset_secs,
                            source: offset_source,
                        }),
                        source_details: SourceDetails {
                            time_source: naive_source,
                            confidence: CONFIDENCE_HIGH.to_string(),
                        },
                    });
                }
            }
        }

        // --- Priority 4: Hybrid (Local Time + Unconfirmed UTC Time) ---
        if let Some((utc_dt, utc_source)) = potential_utc {
            return Some(TimeInfo {
                datetime_utc: Some(utc_dt),
                datetime_local: naive_dt,
                timezone: Some(TimeZoneInfo {
                    name: "UTC".to_string(),
                    offset_seconds: 0,
                    source: utc_source.clone(),
                }),
                source_details: SourceDetails {
                    time_source: format!("{} + {}", naive_source, utc_source),
                    confidence: CONFIDENCE_MEDIUM.to_string(),
                },
            });
        }

        // --- Priority 5: Naive With Guessed Offset ---
        if let Some((file_dt, file_source)) = potential_file_dt {
            let guessed_offset = file_dt.offset().fix();
            let iso_utc = guessed_offset.from_local_datetime(&naive_dt).single().map(|dt| dt.with_timezone(&Utc));

            return Some(TimeInfo {
                datetime_utc: iso_utc,
                datetime_local: naive_dt,
                timezone: Some(TimeZoneInfo {
                    name: guessed_offset.to_string(),
                    offset_seconds: guessed_offset.local_minus_utc(),
                    source: format!("Guessed from {}", file_source),
                }),
                source_details: SourceDetails {
                    time_source: naive_source,
                    confidence: CONFIDENCE_MEDIUM.to_string(),
                },
            });
        }

        // --- Priority 6: Naive Only ---
        return Some(TimeInfo {
            datetime_utc: None,
            datetime_local: naive_dt,
            timezone: None,
            source_details: SourceDetails {
                time_source: naive_source,
                confidence: CONFIDENCE_LOW.to_string(),
            },
        });
    }

    // --- Fallback Path: No authoritative naive time was found anywhere. ---

    // --- Priority 7: Accurate UTC Only ---
    if let Some((utc_dt, utc_source)) = potential_utc {
        return Some(TimeInfo {
            datetime_utc: Some(utc_dt),
            datetime_local: utc_dt.naive_utc(),
            timezone: Some(TimeZoneInfo {
                name: "UTC".to_string(),
                offset_seconds: 0,
                source: utc_source.clone(),
            }),
            source_details: SourceDetails {
                time_source: utc_source,
                confidence: CONFIDENCE_HIGH.to_string(),
            },
        });
    }

    // --- Priority 8: File Metadata Time Only ---
    if let Some((file_dt, file_source)) = potential_file_dt {
        let offset = file_dt.offset().fix();
        return Some(TimeInfo {
            datetime_utc: Some(file_dt.with_timezone(&Utc)),
            datetime_local: file_dt.naive_local(),
            timezone: Some(TimeZoneInfo {
                name: offset.to_string(),
                offset_seconds: offset.local_minus_utc(),
                source: file_source.clone(),
            }),
            source_details: SourceDetails {
                time_source: file_source,
                confidence: CONFIDENCE_LOW.to_string(),
            },
        });
    }

    None
}