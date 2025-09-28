
use crate::tags::structs::TagData;
use crate::time::structs::TimeInfo;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::other::structs::{GpsInfo, MediaMetadata, PanoInfo, WeatherInfo};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnalyzeResult {
    pub metadata: MediaMetadata,
    pub tags: TagData,
    pub exif: Value,
    pub time_info: TimeInfo,
    pub gps_info: Option<GpsInfo>,
    pub weather_info: Option<WeatherInfo>,
    pub pano_info: PanoInfo,
    pub data_url: String,
}
