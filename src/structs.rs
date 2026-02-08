use crate::tags::structs::TagData;
use crate::time::structs::TimeInfo;
use crate::{CaptureDetails, FileMetadata, GpsInfo, PanoInfo};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResult {
    pub hash: String,
    pub exif: Value,
    pub metadata: FileMetadata,
    pub capture_details: CaptureDetails,
    pub tags: TagData,
    pub time_info: TimeInfo,
    pub pano_info: PanoInfo,
    pub gps_info: Option<GpsInfo>,
}
