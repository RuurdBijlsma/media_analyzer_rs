use crate::gps::GpsInfo;
use crate::tags::structs::TagData;
use crate::time::time_types::TimeInfo;
use meteostat::Hourly;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct AnalyzeResult {
    pub tags: TagData,
    pub exif: Value,
    pub time_info: TimeInfo,
    pub gps_info: Option<GpsInfo>,
    pub weather_info: Option<Hourly>,
}
