use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents the extracted and consolidated time information for a media file.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimeInfo {
    /// Timestamp guaranteed to be in UTC (ISO 8601 format with 'Z').
    /// This is the primary field for reliable date/time comparisons and storage.
    /// It's `None` if UTC could not be confidently determined.
    pub datetime_utc: Option<DateTime<Utc>>,

    /// The best available "naive" timestamp (without timezone context) found in the metadata.
    /// This often corresponds to the camera's local time setting when the picture was taken.
    pub datetime_local: NaiveDateTime,

    /// Details about the timezone context associated with `datetime_local`, if determined.
    pub timezone: Option<TimeZoneInfo>,

    /// Information about how the time components were derived
    /// and the overall confidence level.
    pub source_details: SourceDetails,
}

/// Contains details about the timezone determination.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimeZoneInfo {
    /// The name or representation of the timezone.
    /// Can be an IANA name (e.g., "Europe/Bucharest"), a fixed offset string (e.g., "+03:00"),
    /// or simply "UTC".
    pub name: String,
    /// The offset from UTC in seconds.
    /// For IANA zones, this accounts for DST at that time.
    pub offset_seconds: i32,
    /// Describes how the timezone information was obtained (e.g., "IANA from GPS", "`OffsetTimeOriginal`").
    pub source: String,
}

/// Provides context on the origin and reliability of the extracted time information.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceDetails {
    pub time_source: String,
    /// An indicator of the overall reliability of the `TimeInfo` structure,
    /// especially the `datetime_utc` and `timezone` fields.
    pub confidence: String, // e.g., "High", "Medium", "Low"
}

// Confidence level constants
pub const CONFIDENCE_HIGH: &str = "High"; // GPS UTC, Confirmed UTC, Zoned, Explicit Fixed Offset
pub const CONFIDENCE_MEDIUM: &str = "Medium"; // Naive + Guessed Offset
pub const CONFIDENCE_LOW: &str = "Low"; // Naive Only, Filename
