use serde::{Deserialize, Serialize};


/// Tags, such as is_panorama, is_motion_photo, is_night_sight.
#[derive(Debug, PartialEq, Clone,Deserialize, Serialize)]
pub struct TagData {
    pub is_motion_photo: bool,
    pub motion_photo_presentation_timestamp: Option<i64>,
    pub is_night_sight: bool,
    pub is_hdr: bool,
    pub is_burst: bool,
    pub burst_id: Option<String>,
    pub is_timelapse: bool,
    pub is_slowmotion: bool,
    pub is_video: bool,
    pub capture_fps: Option<f64>,
    pub video_fps: Option<f64>,
}
