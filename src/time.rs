use crate::types::gps::GpsInfo;
use crate::types::time::{
    CONFIDENCE_HIGH, CONFIDENCE_LOW, CONFIDENCE_MEDIUM, SourceDetails, TimeInfo, TimeZoneInfo,
};
use chrono::{
    DateTime, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone,
    Timelike, Utc,
};
use chrono_tz::Tz;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;
use std::str::FromStr;
use tzf_rs::DefaultFinder;

// --- Constants ---

// Maximum acceptable difference between naive time from EXIF (like DateTimeOriginal)
// and the naive representation of GPS UTC time for them to be considered consistent.
const MAX_NAIVE_GPS_DIFF_SECONDS: i64 = 5; // Allow up to 5 seconds difference

// --- Global Timezone Finder ---
static FINDER: Lazy<DefaultFinder> = Lazy::new(DefaultFinder::new);

// --- Internal Representation Enum ---
// Helper enum to manage the different ways time can be determined before final formatting.
#[derive(Debug, Clone, PartialEq)]
enum TimeRepresentation {
    UtcConfirmed {
        // Highest confidence: GPS UTC matches Naive source
        datetime: DateTime<Utc>,
        naive_datetime: NaiveDateTime, // The original naive value for reference
        naive_source: String,
        utc_source: String,
    },
    Zoned {
        // Naive time + IANA timezone derived from GPS coordinates
        datetime: DateTime<Tz>,
        naive_source: String,
        iana_source: String,
    },
    FixedOffset {
        // Naive time + explicit fixed offset from EXIF
        datetime: DateTime<FixedOffset>,
        naive_source: String,
        offset_source: String,
    },
    UtcAccurate {
        // UTC time directly from source (e.g., GPSDateTime), no separate naive source available or consistent
        datetime: DateTime<Utc>,
        utc_source: String,
    },
    NaiveWithGuess {
        // Naive time + a guessed offset (usually from file modification time)
        naive_datetime: NaiveDateTime,
        naive_source: String,
        guessed_offset: (FixedOffset, String), // Offset and its source description
    },
    NaiveOnly {
        // Only a naive time is known, no reliable offset or UTC.
        naive_datetime: NaiveDateTime,
        naive_source: String, // Could be DateTimeOriginal, FileName etc.
    },
}

// --- Main Public Function ---

/// Extracts the most reliable time information from EXIF data, optionally using GPS info.
///
/// Prioritizes sources in roughly this order:
/// 1. Confirmed UTC: GPS UTC time consistent with a primary naive timestamp (e.g., DateTimeOriginal).
/// 2. Zoned Time: Primary naive timestamp + IANA timezone derived from GPS coordinates.
/// 3. Fixed Offset Time: Primary naive timestamp + explicit EXIF offset tag (e.g., OffsetTimeOriginal).
/// 4. Accurate UTC: Direct UTC timestamp from EXIF (e.g., GPSDateTime) when no consistent naive time is found.
/// 5. Naive with Guessed Offset: Primary naive timestamp + offset guessed from file metadata (e.g., FileModifyDate).
/// 6. Naive Only: Primary naive timestamp with no reliable offset.
/// 7. Filename Time: Naive timestamp parsed from the filename as a last resort.
///
/// Returns `None` if no usable time information can be extracted.
pub fn get_time_info(exif_info: &Value, gps_info: Option<&GpsInfo>) -> Option<TimeInfo> {
    // 1. Extract all potential time components from EXIF
    let components = extract_time_components(exif_info);

    // 2. Determine the best internal time representation based on available components and priority
    let internal_repr = determine_internal_representation(components, gps_info, exif_info);

    // 3. Convert the internal representation to the final JSON-friendly TimeInfo struct
    internal_repr.map(convert_to_output_info)
}

// --- Stage 1: Extraction ---

// Helper struct to hold extracted time components
struct ExtractedTimeComponents {
    best_naive: Option<(NaiveDateTime, String)>, // (DateTime, Source Tag Name)
    potential_utc: Option<(DateTime<Utc>, String)>, // (DateTime, Source Tag Name)
    potential_explicit_offset: Option<(i32, String, String)>, // (Offset Seconds, Offset String, Source Tag Name)
    potential_file_dt: Option<(DateTime<FixedOffset>, String)>, // (DateTime, Source Tag Name)
}

/// Extracts various time-related values from the raw EXIF JSON data.
fn extract_time_components(exif_info: &Value) -> ExtractedTimeComponents {
    let mut potential_utc: Option<(DateTime<Utc>, String)> = None;
    let mut potential_explicit_offset: Option<(i32, String, String)> = None;
    let mut potential_file_dt: Option<(DateTime<FixedOffset>, String)> = None;

    // --- Best Naive Time (DateTimeOriginal, CreateDate, etc.) with Subseconds ---
    let naive_sources_priority = [
        // Prioritize tags that explicitly mention subseconds in their name first
        ("Time", "SubSecDateTimeOriginal", true), // Example tag, might vary
        ("Time", "SubSecCreateDate", true),
        ("Time", "SubSecTimeDigitized", true),
        // Then standard tags
        ("Time", "DateTimeOriginal", false),
        ("Time", "CreateDate", false),
        ("Time", "DateTimeDigitized", false),
        // Modify dates have lower priority
        ("Time", "SubSecModifyDate", true),
        ("Time", "ModifyDate", false),
    ];

    let mut primary_naive_candidate: Option<(NaiveDateTime, String)> = None;
    let mut found_subsecond_number_source: Option<(String, u32)> = None; // (Field Name, Value)

    for (group, field, _is_subsec_field) in naive_sources_priority {
        // Find the first valid naive datetime string
        if primary_naive_candidate.is_none() {
            if let Some(dt_str) = get_string_field(exif_info, group, field) {
                if let Some((dt, parsed_subsec)) = parse_naive(dt_str) {
                    let source_name = field.to_string();
                    primary_naive_candidate = Some((dt, source_name));
                    // If subseconds were parsed directly from the string, note it in the source
                    if parsed_subsec {
                        found_subsecond_number_source = Some((format!("{}: Parsed", field), 0)); // Mark as found
                    }
                }
            }
        }

        // Once a candidate is found, look for a *separate* numeric subsecond field
        // only if subseconds weren't already parsed from the string.
        if primary_naive_candidate.is_some() && found_subsecond_number_source.is_none() {
            let base_field_name = field.replace("SubSec", ""); // e.g., DateTimeOriginal
            let sub_sec_num_field = format!(
                "SubSecTime{}",
                base_field_name.replace("Date", "").replace("Time", "") // e.g., SubSecTimeOriginal
            );

            if let Some(subsec_num) = get_number_field(exif_info, group, &sub_sec_num_field) {
                // Ensure this subsecond field corresponds to the found naive field
                if primary_naive_candidate
                    .as_ref()
                    .is_some_and(|(_, src)| *src == base_field_name || *src == field)
                {
                    found_subsecond_number_source = Some((sub_sec_num_field, subsec_num));
                }
            }
            // Also check for a simpler SubSec field if the complex one isn't found
            let simpler_sub_sec_field = format!("{}SubSecond", base_field_name); // e.g. DateTimeOriginalSubSecond (less common?)
            if found_subsecond_number_source.is_none() {
                if let Some(subsec_num) = get_number_field(exif_info, group, &simpler_sub_sec_field)
                {
                    if primary_naive_candidate
                        .as_ref()
                        .is_some_and(|(_, src)| *src == base_field_name || *src == field)
                    {
                        found_subsecond_number_source = Some((simpler_sub_sec_field, subsec_num));
                    }
                }
            }
        }

        // Stop searching if we have a naive candidate and have checked for its subseconds
        if primary_naive_candidate.is_some() && found_subsecond_number_source.is_some() {
            break;
        }
        // Or stop if we have a naive candidate and have exhausted the priority list (no subsecond field found)
        if primary_naive_candidate.is_some() && field == naive_sources_priority.last().unwrap().1 {
            break;
        }
    }

    // Apply numeric subseconds if found and not parsed from string
    if let (Some((naive_dt, source_name)), Some((subsec_source, subsec_num))) = (
        primary_naive_candidate.as_mut(),
        found_subsecond_number_source.as_ref(),
    ) {
        if !subsec_source.ends_with(": Parsed") {
            // Avoid applying if already parsed from string
            *naive_dt = add_subseconds_from_number(*naive_dt, *subsec_num);
            // Append subsecond source info
            *source_name = format!("{} + {}", source_name, subsec_source);
        } else {
            // Just clarify the source name if parsed directly
            *source_name = format!("{}: Parsed SubSeconds", source_name);
        }
    }
    let best_naive = primary_naive_candidate;

    // --- Potential UTC Time (GPSDateTime, GPSDateStamp/TimeStamp) ---
    if let Some(gps_dt_str) = get_string_field(exif_info, "Time", "GPSDateTime") {
        if let Some(dt_utc) = parse_datetime_utc_z(gps_dt_str) {
            potential_utc = Some((dt_utc, "GPSDateTime".to_string()));
        }
    }
    // Fallback to separate GPS Date and Time stamps if GPSDateTime not found/parsed
    if potential_utc.is_none() {
        if let (Some(date_str), Some(time_str)) = (
            get_string_field(exif_info, "Time", "GPSDateStamp"),
            get_string_field(exif_info, "Time", "GPSTimeStamp"),
        ) {
            // Combine date and time, assuming UTC ('Z')
            let combined_str = format!("{} {}Z", date_str, time_str);
            if let Some(dt_utc) = parse_datetime_utc_z(&combined_str) {
                potential_utc = Some((dt_utc, "GPSDateStamp/GPSTimeStamp".to_string()));
            }
        }
    }

    // --- Potential Explicit Offset (OffsetTimeOriginal, etc.) ---
    let offset_sources_priority = [
        ("Time", "OffsetTimeOriginal"),
        ("Time", "OffsetTimeDigitized"),
        ("Time", "OffsetTime"), // General offset
    ];
    for (group, field) in offset_sources_priority {
        if let Some(offset_str) = get_string_field(exif_info, group, field) {
            if let Some((secs, parsed_str)) = parse_offset_string(offset_str) {
                potential_explicit_offset = Some((secs, parsed_str, field.to_string()));
                break; // Found the highest priority offset
            }
        }
    }

    // --- Potential File Time (FileModifyDate, etc.) - Used for offset guessing ---
    let file_time_sources_priority = [
        ("Time", "FileModifyDate"), // Often reflects local time with offset
        ("Time", "FileCreateDate"), // Less reliable for photo time
        ("Time", "FileAccessDate"), // Least reliable
    ];
    for (group, field) in file_time_sources_priority {
        if let Some(dt_str) = get_string_field(exif_info, group, field) {
            if let Some(dt) = parse_datetime_offset(dt_str) {
                potential_file_dt = Some((dt, field.to_string()));
                break; // Found the highest priority file time
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

// --- Stage 2: Determination Logic ---

/// Determines the most reliable time representation based on extracted components and priority rules.
fn determine_internal_representation(
    components: ExtractedTimeComponents,
    gps_info: Option<&GpsInfo>,
    exif_info: &Value, // Needed for filename fallback
) -> Option<TimeRepresentation> {
    let ExtractedTimeComponents {
        best_naive,
        potential_utc,
        potential_explicit_offset,
        potential_file_dt,
    } = components;

    // --- Priority 1: Confirmed UTC ---
    // Check if we have both a reliable naive source (like DateTimeOriginal) and a GPS UTC source,
    // and if their naive components are consistent.
    if let (Some((naive_dt, naive_source)), Some((utc_dt, utc_source))) =
        (&best_naive, &potential_utc)
    {
        let naive_from_utc = utc_dt.naive_utc();
        let diff = naive_from_utc.signed_duration_since(*naive_dt);

        // Check if the difference is within the acceptable threshold
        if diff.num_seconds().abs() <= MAX_NAIVE_GPS_DIFF_SECONDS {
            return Some(TimeRepresentation::UtcConfirmed {
                datetime: *utc_dt,
                naive_datetime: *naive_dt, // Keep the original naive for reference if needed
                naive_source: naive_source.clone(),
                utc_source: utc_source.clone(),
            });
        }
        // If they differ significantly, we'll proceed to other priorities, potentially favoring GPSDateTime later.
    }

    // --- Priority 2: Zoned Time (Naive + GPS Location for IANA TZ) ---
    if let Some((naive_dt, ref naive_source)) = best_naive {
        if let Some(gps) = gps_info {
            // Use tzf-rs to find the IANA timezone name from coordinates
            let tz_name = FINDER.get_tz_name(gps.longitude, gps.latitude);
            if let Ok(tz) = Tz::from_str(tz_name) {
                // Convert the naive datetime to this timezone. Handle ambiguity (e.g., DST changes)
                // by preferring the earlier time, which is a common convention.
                if let LocalResult::Single(zoned_dt) | LocalResult::Ambiguous(zoned_dt, _) =
                    tz.from_local_datetime(&naive_dt)
                {
                    return Some(TimeRepresentation::Zoned {
                        datetime: zoned_dt,
                        naive_source: naive_source.clone(),
                        iana_source: "IANA from GPS".to_string(),
                    });
                }
            }
        }

        // --- Priority 3: Fixed Offset Time (Naive + Explicit EXIF Offset) ---
        if let Some((offset_secs, _offset_str, ref offset_source)) = potential_explicit_offset {
            if let Some(offset) = FixedOffset::east_opt(offset_secs) {
                // Convert naive time using the fixed offset. Handle ambiguity if needed.
                if let LocalResult::Single(dt_with_offset)
                | LocalResult::Ambiguous(dt_with_offset, _) =
                    offset.from_local_datetime(&naive_dt)
                {
                    return Some(TimeRepresentation::FixedOffset {
                        datetime: dt_with_offset,
                        naive_source: naive_source.clone(),
                        offset_source: offset_source.clone(),
                    });
                }
            }
        }
        // Fall through if Zoned/FixedOffset didn't apply to this 'best_naive'
    }

    // --- Priority 4: Accurate UTC (Direct GPS Time, no consistent naive found) ---
    // This catches cases where GPSDateTime exists but DateTimeOriginal is missing,
    // or where they existed but were inconsistent (checked in Priority 1).
    if let Some((utc_dt, utc_source)) = potential_utc {
        return Some(TimeRepresentation::UtcAccurate {
            datetime: utc_dt,
            utc_source,
        });
    }

    // --- Priority 5: Naive With Guessed Offset (from File Time) ---
    // This is reached if we have a 'best_naive' but couldn't determine a reliable zone/offset,
    // and there was no GPS UTC time either.
    if let Some((naive_dt, naive_source)) = best_naive {
        return if let Some((file_dt, file_source)) = potential_file_dt {
            let guessed_offset = file_dt.offset().fix(); // Get the offset from the file time
            let guessed_source = format!("Guessed from {}", file_source);
            Some(TimeRepresentation::NaiveWithGuess {
                naive_datetime: naive_dt,
                naive_source,
                guessed_offset: (guessed_offset, guessed_source),
            })
        } else {
            // --- Priority 6: Naive Only ---
            // If we have 'best_naive' but absolutely no offset info (explicit or guessed).
            Some(TimeRepresentation::NaiveOnly {
                naive_datetime: naive_dt,
                naive_source,
            })
        };
    }

    // --- Priority 7: Filename Parsing (Last Resort) ---
    if let Some(filename) = get_string_field(exif_info, "Other", "FileName") {
        // Regex to capture YYYYMMDD_HHMMSS or similar formats
        // Using \b for word boundaries to avoid matching parts of longer numbers.
        let re =
            Regex::new(r"\b(\d{4})[-_]?(\d{2})[-_]?(\d{2})[-_ ]?(\d{2})[:_]?(\d{2})[:_]?(\d{2})\b")
                .ok()?;
        if let Some(caps) = re.captures(filename) {
            // Attempt to parse all captured groups as numbers
            if let (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(min), Ok(sec)) = (
                caps.get(1)?.as_str().parse::<i32>(),
                caps.get(2)?.as_str().parse::<u32>(),
                caps.get(3)?.as_str().parse::<u32>(),
                caps.get(4)?.as_str().parse::<u32>(),
                caps.get(5)?.as_str().parse::<u32>(),
                caps.get(6)?.as_str().parse::<u32>(),
            ) {
                // Validate and construct NaiveDateTime
                if let (Some(date), Some(time)) = (
                    NaiveDate::from_ymd_opt(year, month, day),
                    NaiveTime::from_hms_opt(hour, min, sec),
                ) {
                    // Treat filename time as NaiveOnly with a specific source
                    return Some(TimeRepresentation::NaiveOnly {
                        naive_datetime: NaiveDateTime::new(date, time),
                        naive_source: "FileName".to_string(),
                    });
                }
            }
        }
    }

    // --- No Time Found ---
    None
}

// --- Stage 3: Conversion to Output Format ---

/// Converts the internal `TimeRepresentation` into the final `TimeInfo` struct.
fn convert_to_output_info(repr: TimeRepresentation) -> TimeInfo {
    match repr {
        TimeRepresentation::UtcConfirmed {
            datetime,
            naive_datetime,
            naive_source,
            utc_source,
        } => {
            // We have high confidence UTC, derived from matching GPS and naive sources.
            let tz_info = TimeZoneInfo {
                name: "UTC".to_string(),
                offset_seconds: 0,
                source: format!("{} confirmed by {}", utc_source, naive_source), // More descriptive source
            };
            TimeInfo {
                datetime_utc: Some(datetime),
                datetime_naive: naive_datetime, // Use the original naive time found
                timezone: Some(tz_info),
                source_details: SourceDetails {
                    time_source: naive_source, // Primary source is the naive tag
                    confidence: CONFIDENCE_HIGH.to_string(),
                },
            }
        }
        TimeRepresentation::Zoned {
            datetime,
            naive_source,
            iana_source,
        } => {
            // Timezone found via GPS lookup (tzf-rs)
            let utc_dt = datetime.with_timezone(&Utc);
            let offset_secs = datetime.offset().fix().local_minus_utc();
            TimeInfo {
                datetime_utc: Some(utc_dt),
                datetime_naive: datetime.naive_local(), // Naive representation in the derived zone
                timezone: Some(TimeZoneInfo {
                    name: datetime.timezone().name().to_string(), // IANA name
                    offset_seconds: offset_secs,
                    source: iana_source,
                }),
                source_details: SourceDetails {
                    time_source: naive_source,
                    confidence: CONFIDENCE_HIGH.to_string(),
                },
            }
        }
        TimeRepresentation::FixedOffset {
            datetime,
            naive_source,
            offset_source,
        } => {
            // Timezone determined by an explicit offset tag in EXIF
            let utc_dt = datetime.with_timezone(&Utc);
            let offset_secs = datetime.offset().fix().local_minus_utc();
            TimeInfo {
                datetime_utc: Some(utc_dt),
                datetime_naive: datetime.naive_local(), // Naive representation in the fixed offset
                timezone: Some(TimeZoneInfo {
                    name: datetime.offset().to_string(), // Format like "+03:00"
                    offset_seconds: offset_secs,
                    source: offset_source, // e.g., "OffsetTimeOriginal"
                }),
                source_details: SourceDetails {
                    time_source: naive_source,
                    confidence: CONFIDENCE_HIGH.to_string(),
                },
            }
        }
        TimeRepresentation::UtcAccurate {
            datetime,
            utc_source,
        } => {
            // Directly using a UTC source like GPSDateTime, as naive source was missing or inconsistent
            TimeInfo {
                datetime_utc: Some(datetime),
                // For a pure UTC source, the 'naive' representation *is* the UTC time numerically
                datetime_naive: datetime.naive_utc(),
                timezone: Some(TimeZoneInfo {
                    name: "UTC".to_string(),
                    offset_seconds: 0,
                    source: utc_source.clone(), // e.g., "GPSDateTime"
                }),
                source_details: SourceDetails {
                    // The source of the 'naive' time is effectively the UTC source itself here
                    time_source: utc_source,
                    confidence: CONFIDENCE_HIGH.to_string(),
                },
            }
        }
        TimeRepresentation::NaiveWithGuess {
            naive_datetime,
            naive_source,
            guessed_offset,
        } => {
            // We have a naive time and an offset guessed from file metadata
            let (offset, offset_source) = guessed_offset;
            let mut iso_utc: Option<DateTime<Utc>> = None;

            // Attempt to calculate UTC using the guessed offset. This might fail (e.g., invalid times)
            // or be ambiguous during DST transitions. We take the result if single or ambiguous.
            if let LocalResult::Single(guessed_dt_offset)
            | LocalResult::Ambiguous(guessed_dt_offset, _) =
                offset.from_local_datetime(&naive_datetime)
            {
                iso_utc = Some(guessed_dt_offset.with_timezone(&Utc));
            }

            let json_tz = Some(TimeZoneInfo {
                name: offset.to_string(), // Format like "+03:00"
                offset_seconds: offset.local_minus_utc(),
                source: offset_source, // e.g., "Guessed from FileModifyDate"
            });

            TimeInfo {
                datetime_utc: iso_utc, // This *might* be None if the conversion failed
                datetime_naive: naive_datetime,
                timezone: json_tz,
                source_details: SourceDetails {
                    time_source: naive_source,
                    confidence: CONFIDENCE_MEDIUM.to_string(), // Confidence is medium due to guess
                },
            }
        }
        TimeRepresentation::NaiveOnly {
            naive_datetime,
            naive_source,
        } => {
            // Only have a naive datetime, cannot determine UTC or offset reliably
            TimeInfo {
                datetime_utc: None, // Cannot determine UTC
                datetime_naive: naive_datetime,
                timezone: None, // No timezone info
                source_details: SourceDetails {
                    time_source: naive_source, // Could be "DateTimeOriginal", "FileName", etc.
                    confidence: CONFIDENCE_LOW.to_string(), // Confidence is low
                },
            }
        }
    }
}

// --- Helper Functions ---

/// Safely extracts a string field from nested JSON Value.
fn get_string_field<'a>(value: &'a Value, group: &str, field: &str) -> Option<&'a str> {
    value.get(group)?.get(field)?.as_str()
}

/// Safely extracts a number field (as u32) from nested JSON Value.
fn get_number_field(value: &Value, group: &str, field: &str) -> Option<u32> {
    value
        .get(group)?
        .get(field)?
        .as_u64()
        .and_then(|n| u32::try_from(n).ok()) // Convert u64 -> u32 safely
}

/// Parses an offset string like "+02:00", "-0500", or "Z" into offset seconds and the original string.
fn parse_offset_string(offset_str: &str) -> Option<(i32, String)> {
    // Handle UTC 'Z' explicitly
    if offset_str == "Z" {
        return Some((0, "UTC".to_string()));
    }
    // Regex for +HH:MM, +HHMM, -HH:MM, -HHMM
    let re_offset = Regex::new(r"^([+-])(\d{2}):?(\d{2})$").ok()?;
    if let Some(caps) = re_offset.captures(offset_str) {
        let sign = if caps.get(1)?.as_str() == "-" { -1 } else { 1 };
        // Use fallible parsing
        let hours = caps.get(2)?.as_str().parse::<i32>().ok()?;
        let minutes = caps.get(3)?.as_str().parse::<i32>().ok()?;
        // Basic validation for hours/minutes
        if hours > 14 || minutes > 59 {
            // Offsets beyond +/-14:00 are invalid
            return None;
        }
        let total_secs = sign * (hours * 3600 + minutes * 60);
        return Some((total_secs, offset_str.to_string()));
    }
    None
}

/// Parses a naive datetime string commonly found in EXIF (YYYY:MM:DD HH:MM:SS[.fff]).
/// Returns the NaiveDateTime and a boolean indicating if subseconds were present in the string.
fn parse_naive(s: &str) -> Option<(NaiveDateTime, bool)> {
    // Try parsing with fractional seconds first
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S%.f") {
        Some((dt, dt.nanosecond() != 0)) // Check if nanos are non-zero
    } else if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S") {
        Some((dt, false)) // No fractional part in format string
    } else {
        None // Failed to parse either format
    }
}

/// Parses a datetime string with a timezone offset (e.g., file modification date).
fn parse_datetime_offset(s: &str) -> Option<DateTime<FixedOffset>> {
    // Chrono's standard parser for RFC 3339 / ISO 8601 style offsets
    DateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%S%z").ok()
}

/// Parses a datetime string ending in 'Z' indicating UTC.
fn parse_datetime_utc_z(s: &str) -> Option<DateTime<Utc>> {
    // Parse as DateTime<FixedOffset> first, as %Z expects 'Z'
    DateTime::parse_from_str(s, "%Y:%m:%d %H:%M:%SZ")
        .ok()
        .map(|dt| dt.with_timezone(&Utc)) // Convert to DateTime<Utc>
}

/// Adds subsecond precision from a separate numeric EXIF field to a NaiveDateTime.
/// Assumes the number represents fractions of a second (e.g., 123 means 0.123s).
fn add_subseconds_from_number(dt: NaiveDateTime, subsec_num: u32) -> NaiveDateTime {
    if subsec_num == 0 {
        return dt;
    } // No change if zero

    let subsec_str = subsec_num.to_string();
    let num_digits = subsec_str.len() as u32;

    // Calculate nanoseconds based on the number of digits provided.
    // E.g., if subsec_num is 12 (2 digits), it represents 0.12s or 120,000,000 ns.
    // Formula: subsec_num * 10^(9 - num_digits)
    let nanos = if num_digits <= 9 {
        subsec_num.saturating_mul(10u32.pow(9u32.saturating_sub(num_digits)))
    } else {
        // If more than 9 digits provided (unlikely), treat as nanoseconds directly, modulo 1 billion
        subsec_num % 1_000_000_000
    };

    // Use `with_nanosecond` which safely adds nanoseconds to the existing time.
    // `unwrap_or(dt)` handles potential overflow, though unlikely with valid times/nanos.
    dt.with_nanosecond(nanos).unwrap_or(dt)
}
