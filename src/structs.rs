use crate::other::gps::GpsInfo;
use crate::other::metadata::{CaptureDetails, FileMetadata};
use crate::other::pano::PanoInfo;
use crate::other::weather::WeatherInfo;
use crate::tags::structs::TagData;
use crate::time::structs::TimeInfo;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnalyzeResult {
    pub exif: Value,
    pub metadata: FileMetadata,
    pub capture_details: CaptureDetails,
    pub tags: TagData,
    pub time_info: TimeInfo,
    pub pano_info: PanoInfo,
    pub data_url: String,
    pub gps_info: Option<GpsInfo>,
    pub weather_info: Option<WeatherInfo>,
}
