use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq,Deserialize, Serialize)]
pub struct PanoInfo {
    /// The calculated horizontal field of view in degrees.
    pub horizontal_fov_deg: f64,
    /// The calculated vertical field of view in degrees.
    pub vertical_fov_deg: f64,
    /// The horizontal center of the view in degrees (-180 to 180).
    pub center_yaw_deg: f64,
    /// The vertical center of the view in degrees (-90 to 90).
    pub center_pitch_deg: f64,
}

/// Tags, such as is_panorama, is_motion_photo, is_night_sight.
#[derive(Debug, PartialEq, Clone,Deserialize, Serialize)]
pub struct TagData {
    pub use_panorama_viewer: bool,
    pub pano_info: Option<PanoInfo>,
    pub is_photosphere: bool,
    pub projection_type: Option<String>,
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
