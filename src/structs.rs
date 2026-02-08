use crate::tags::structs::MediaFeatures;
use crate::time::structs::TimeInfo;
use crate::{BasicMetadata, CameraSettings, GpsInfo, PanoInfo, WeatherInfo};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadata {
    pub hash: String,
    pub exif: Value,
    pub basic: BasicMetadata,
    pub camera: CameraSettings,
    pub features: MediaFeatures,
    pub time: TimeInfo,
    pub panorama: PanoInfo,
    pub gps: Option<GpsInfo>,
    pub weather: Option<WeatherInfo>,
}
