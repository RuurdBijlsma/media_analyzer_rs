//! Core logic for determining the best time representation based on extracted components.

use super::error::TimeError;
use super::extraction::{ExtractedTimeComponents, extract_time_components};
use crate::GpsInfo;
use crate::time::structs::{
    CONFIDENCE_HIGH, CONFIDENCE_LOW, CONFIDENCE_MEDIUM, SourceDetails, TimeInfo, TimeZoneInfo,
};
use chrono::{FixedOffset, LocalResult, Offset, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::Value;
use std::str::FromStr;
use tzf_rs::DefaultFinder;

// --- Constants specific to the logic ---
const MAX_NAIVE_GPS_DIFF_SECONDS: i64 = 10;

// --- Global Timezone Finder ---
static FINDER: std::sync::LazyLock<DefaultFinder> = std::sync::LazyLock::new(DefaultFinder::new);

pub fn get_time_info(exif_info: &Value, gps_info: Option<&GpsInfo>) -> Result<TimeInfo, TimeError> {
    let components = extract_time_components(exif_info);
    let time_result = apply_priority_logic(components, gps_info);
    time_result.ok_or(TimeError::Extraction)
}

/// Applies the priority logic to extracted components and constructs the final `TimeInfo`.
fn apply_priority_logic(
    components: ExtractedTimeComponents,
    gps_info: Option<&GpsInfo>,
) -> Option<TimeInfo> {
    let ExtractedTimeComponents {
        best_local,
        potential_utc,
        potential_explicit_offset,
        potential_file_dt,
    } = components;

    // --- Priority 1: Confirmed UTC (Highest confidence) ---
    if let (Some((local_dt, naive_source)), Some((gps_utc_dt, utc_source)), Some(gps)) =
        (&best_local, &potential_utc, gps_info)
        && let Ok(tz) = Tz::from_str(FINDER.get_tz_name(gps.longitude, gps.latitude))
        && let LocalResult::Single(zoned_dt) | LocalResult::Ambiguous(zoned_dt, _) =
            tz.from_local_datetime(local_dt)
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
                datetime_local: *local_dt,
                timezone: Some(tz_info),
                source_details: SourceDetails {
                    time_source: naive_source.clone(),
                    confidence: CONFIDENCE_HIGH.to_string(),
                },
            });
        }
    }

    if let Some((local_dt, naive_source)) = best_local {
        // --- Priority 2: Zoned Time (Naive + GPS Location) ---
        if let Some(gps) = gps_info
            && let Ok(tz) = Tz::from_str(FINDER.get_tz_name(gps.longitude, gps.latitude))
            && let LocalResult::Single(zoned_dt) | LocalResult::Ambiguous(zoned_dt, _) =
                tz.from_local_datetime(&local_dt)
        {
            return Some(TimeInfo {
                datetime_utc: Some(zoned_dt.with_timezone(&Utc)),
                datetime_local: local_dt,
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

        // --- Priority 3: Fixed Offset Time (Naive + Explicit Offset Tag) ---
        if let Some((offset_secs, offset_str, offset_source)) = potential_explicit_offset
            && let Some(offset) = FixedOffset::east_opt(offset_secs)
            && let LocalResult::Single(dt_with_offset) | LocalResult::Ambiguous(dt_with_offset, _) =
                offset.from_local_datetime(&local_dt)
        {
            return Some(TimeInfo {
                datetime_utc: Some(dt_with_offset.with_timezone(&Utc)),
                datetime_local: local_dt,
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

        // --- Priority 4: Hybrid (Local Time + Unconfirmed UTC Time) ---
        if let Some((utc_dt, utc_source)) = potential_utc {
            // Calculate offset in seconds between local and UTC datetimes
            let offset_seconds = (local_dt - utc_dt.naive_utc()).num_seconds() as i32;

            // Format offset as ±HH:MM
            let sign = if offset_seconds >= 0 { '+' } else { '-' };
            let abs_offset = offset_seconds.abs();
            let hours = abs_offset / 3600;
            let minutes = (abs_offset % 3600) / 60;
            let tz_name = format!("{sign}{hours:02}:{minutes:02}");

            return Some(TimeInfo {
                datetime_utc: Some(utc_dt),
                datetime_local: local_dt,
                timezone: Some(TimeZoneInfo {
                    name: tz_name,
                    offset_seconds,
                    source: utc_source.clone(),
                }),
                source_details: SourceDetails {
                    time_source: format!("{naive_source} + {utc_source}"),
                    confidence: CONFIDENCE_MEDIUM.to_string(),
                },
            });
        }

        // --- Priority 5: Naive With Guessed Offset ---
        if let Some((file_dt, file_source)) = potential_file_dt {
            let guessed_offset = file_dt.offset().fix();
            let iso_utc = guessed_offset
                .from_local_datetime(&local_dt)
                .single()
                .map(|dt| dt.with_timezone(&Utc));

            return Some(TimeInfo {
                datetime_utc: iso_utc,
                datetime_local: local_dt,
                timezone: Some(TimeZoneInfo {
                    name: guessed_offset.to_string(),
                    offset_seconds: guessed_offset.local_minus_utc(),
                    source: format!("Guessed from {file_source}"),
                }),
                source_details: SourceDetails {
                    time_source: naive_source,
                    confidence: CONFIDENCE_MEDIUM.to_string(),
                },
            });
        }

        // we are left with just the naive time.
        return Some(TimeInfo {
            datetime_utc: None,
            datetime_local: local_dt,
            timezone: None,
            source_details: SourceDetails {
                time_source: naive_source,
                confidence: CONFIDENCE_LOW.to_string(),
            },
        });
    }

    // --- Fallback Path: No authoritative naive time was found anywhere. ---

    // --- Priority 7: Accurate UTC Only, no naive time available somehow. ---
    if let Some((utc_dt, utc_source)) = potential_utc {
        // No tz available
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::gps::get_gps_info;
    use crate::{LocationName, MediaAnalyzerError};
    use chrono::NaiveDate;
    use exiftool::ExifTool;
    use reverse_geocoder::ReverseGeocoder;
    use serde_json::from_str;
    use std::path::Path;

    // Mock GpsInfo struct as it's defined in another part of the crate.
    #[derive(Debug, Clone, Copy)]
    pub struct MockGpsInfo {
        pub latitude: f64,
        pub longitude: f64,
    }
    // Allow tests to use the mock struct where the real GpsInfo is expected.
    impl From<MockGpsInfo> for GpsInfo {
        fn from(val: MockGpsInfo) -> Self {
            Self {
                latitude: val.latitude,
                longitude: val.longitude,
                altitude: None,
                image_direction: None,
                image_direction_ref: None,
                location: LocationName {
                    latitude: 0.0,
                    name: String::new(),
                    admin1: String::new(),
                    admin2: String::new(),
                    country_code: String::new(),
                    longitude: 0.,
                    country_name: None,
                },
            }
        }
    }

    fn get_full_exif() -> Value {
        from_str(r#"{
            "Time": {
                "FileModifyDate": "2025:02:26 19:14:06+01:00",
                "ModifyDate": "2017:11:06 11:03:20",
                "GPSDateStamp": "2017:11:06",
                "GPSTimeStamp": "10:03:19",
                "CreateDate": "2017:11:06 11:03:20",
                "SubSecTimeOriginal": 123953,
                "DateTimeOriginal": "2017:11:06 11:03:20",
                "SubSecDateTimeOriginal": "2017:11:06 11:03:20.123953",
                "GPSDateTime": "2017:11:06 10:03:19Z"
            },
            "Location": { "GPSLatitude": "53 deg 12' 45.68\" N", "GPSLongitude": "6 deg 33' 46.93\" E" }
            }"#).unwrap()
    }

    fn get_basic_exif() -> Value {
        from_str(
            r#"{
        "Time": {
        "FileModifyDate": "2011:01:01 15:26:40+01:00",
        "ModifyDate": "2011:01:01 16:26:30",
        "DateTimeOriginal": "                    ",
        "CreateDate": "                    "
        }
        }"#,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_difficult_tz_offset() -> Result<(), MediaAnalyzerError> {
        // Arrange
        let image = Path::new("assets/tz-offset-bug/IMG_20170904_101507.jpg");
        let et = ExifTool::new()?;
        let exif = et.json(image, &["-g2"])?;
        let geocoder = ReverseGeocoder::new();
        let numeric_exif = et.json(image, &["-n"])?;
        let gps_info = get_gps_info(&geocoder, &numeric_exif).await;

        // Act
        let time_info = get_time_info(&exif, gps_info.as_ref())?;

        // Assert
        println!("{time_info:?}");

        Ok(())
    }

    #[test]
    fn test_priority1_confirmed_utc_from_cluster_jpg() {
        let exif = get_full_exif();
        // GPS Coordinates for Groningen, NL
        let gps = MockGpsInfo {
            latitude: 53.212_688,
            longitude: 6.563_036,
        };

        let info = get_time_info(&exif, Some(&gps.into())).unwrap();

        // UTC time should come directly from GPSDateTime because it's confirmed.
        assert_eq!(
            info.datetime_utc.unwrap().to_rfc3339(),
            "2017-11-06T10:03:19+00:00"
        );
        // Local time is the high-precision naive time from SubSecDateTimeOriginal.
        assert_eq!(
            info.datetime_local,
            NaiveDate::from_ymd_opt(2017, 11, 6)
                .unwrap()
                .and_hms_micro_opt(11, 3, 20, 123_953)
                .unwrap()
        );
        // Timezone should be identified from GPS and used for confirmation.
        assert_eq!(info.timezone.as_ref().unwrap().name, "Europe/Amsterdam");
        assert_eq!(info.timezone.as_ref().unwrap().offset_seconds, 3600);
        // Confidence should be high.
        assert_eq!(info.source_details.confidence, CONFIDENCE_HIGH);
        assert!(info.timezone.unwrap().source.contains("confirmed by"));
    }

    #[test]
    fn test_priority5_guessed_offset_from_pict0017() {
        let exif = get_basic_exif();
        // No GPS, no fallback timezone.
        let info = get_time_info(&exif, None).unwrap();

        // `best_local` comes from `ModifyDate` since `DateTimeOriginal` is blank.
        assert_eq!(
            info.datetime_local,
            NaiveDate::from_ymd_opt(2011, 1, 1)
                .unwrap()
                .and_hms_opt(16, 26, 30)
                .unwrap()
        );
        // The offset is "guessed" from the `FileModifyDate`.
        assert_eq!(info.timezone.as_ref().unwrap().name, "+01:00");
        assert_eq!(info.timezone.as_ref().unwrap().offset_seconds, 3600);
        assert_eq!(info.timezone.unwrap().source, "Guessed from FileModifyDate");
        // The UTC time is calculated from the local time + guessed offset.
        assert_eq!(
            info.datetime_utc.unwrap().to_rfc3339(),
            "2011-01-01T15:26:30+00:00"
        );
        // Confidence is Medium because the offset is a guess.
        assert_eq!(info.source_details.confidence, CONFIDENCE_MEDIUM);
    }

    #[test]
    fn test_priority6_naive_with_fallback_timezone() {
        let exif = get_basic_exif();
        let info = get_time_info(&exif, None).unwrap();

        assert_eq!(
            info.datetime_local,
            NaiveDate::from_ymd_opt(2011, 1, 1)
                .unwrap()
                .and_hms_opt(16, 26, 30)
                .unwrap()
        );
        // The timezone is now the provided fallback.
        assert_eq!(info.timezone.as_ref().unwrap().name, "+01:00");
        assert_eq!(info.timezone.unwrap().source, "Guessed from FileModifyDate");
        // UTC time calculated from local time + Paris offset in winter (+1).
        assert_eq!(
            info.datetime_utc.unwrap().to_rfc3339(),
            "2011-01-01T15:26:30+00:00"
        );
        // Confidence is "Fallback".
        assert_eq!(info.source_details.confidence, CONFIDENCE_MEDIUM);
    }

    #[test]
    fn test_priority6_naive_only_low_confidence() {
        let exif =
            from_str(r#"{ "Time": { "DateTimeOriginal": "2023-05-10 10:00:00" } }"#).unwrap();
        let info = get_time_info(&exif, None).unwrap();

        assert_eq!(
            info.datetime_local,
            NaiveDate::from_ymd_opt(2023, 5, 10)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap()
        );
        // With no other information, UTC and timezone must be None.
        assert!(info.datetime_utc.is_none());
        assert!(info.timezone.is_none());
        // Confidence is Low.
        assert_eq!(info.source_details.confidence, CONFIDENCE_LOW);
    }

    #[test]
    fn test_priority7_utc_only_with_fallback_timezone() {
        let exif = from_str(r#"{ "Time": { "GPSDateTime": "2022-08-15T18:00:00Z" } }"#).unwrap();
        let info = get_time_info(&exif, None).unwrap();

        // UTC time is known and accurate.
        assert_eq!(
            info.datetime_utc.unwrap().to_rfc3339(),
            "2022-08-15T18:00:00+00:00"
        );
        // The local time should be the UTC time, since no naive time is provided at all.
        assert_eq!(
            info.datetime_local,
            NaiveDate::from_ymd_opt(2022, 8, 15)
                .unwrap()
                .and_hms_opt(18, 0, 0)
                .unwrap()
        );
        assert_eq!(info.timezone.as_ref().unwrap().name, "UTC");
        assert_eq!(info.timezone.as_ref().unwrap().offset_seconds, 0);
        assert_eq!(info.timezone.unwrap().source, "GPSDateTime");
    }
}
