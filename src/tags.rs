use lazy_static::lazy_static;
use regex::Regex;
use serde_json::Value;
use std::path::Path;

/// Tags, such as is_panorama, is_motion_photo, is_night_sight.
#[derive(Debug, PartialEq)]
pub struct TagData {
    pub use_panorama_viewer: bool,
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

/// Detects if a filename indicates it's a burst photo and extracts the burst ID.
fn detect_burst(filename: &str) -> (bool, Option<String>) {
    if !filename.to_lowercase().contains("burst") {
        return (false, None);
    }

    lazy_static! {
        static ref BURST_PATTERN_1: Regex = Regex::new(r"BURST(\d{17})").unwrap();
        static ref BURST_PATTERN_2: Regex = Regex::new(r"(.*?)_Burst\d+").unwrap();
    }

    if let Some(caps) = BURST_PATTERN_1.captures(filename) {
        if let Some(id) = caps.get(1) {
            return (true, Some(id.as_str().to_string()));
        }
    }

    if let Some(caps) = BURST_PATTERN_2.captures(filename) {
        if let Some(id) = caps.get(1) {
            return (true, Some(id.as_str().to_string()));
        }
    }

    (false, None)
}

/// Extracts tags from a file's path and its EXIF metadata.
pub fn extract_tags(path: &Path, exif: &Value) -> TagData {
    let filename_lower = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    // --- Tags from Filename ---
    let (is_burst, burst_id) = detect_burst(&filename_lower);
    let is_night_sight = filename_lower.contains("night");

    // --- Tags from EXIF data ---

    // Check for HDR from filename or from the "Software" tag
    let is_hdr = filename_lower.contains("hdr") || exif
        .get("Image")
        .and_then(|img| img.get("Software"))
        .and_then(|s| s.as_str())
        .map_or(false, |s| s.to_lowercase().starts_with("hdr+"));

    // Check for video from MIME type
    let is_video = exif
        .get("Other")
        .and_then(|o| o.get("MIMEType"))
        .and_then(|m| m.as_str())
        .map_or(false, |s| s.starts_with("video/"));

    // Motion Photo data is often in the "Camera" block for modern phones
    let is_motion_photo = exif
        .get("Camera")
        .and_then(|c| c.get("MotionPhoto"))
        .and_then(|v| v.as_i64())
        == Some(1);

    let motion_photo_presentation_timestamp = if is_motion_photo {
        exif.get("Camera")
            .and_then(|c| c.get("MotionPhotoPresentationTimestampUs"))
            .and_then(|v| v.as_i64())
    } else {
        None
    };

    // TODO:
    // * photosphere
    // * use panorama viewer
    // * projection_type

    // TODO - VIDEO:
    // * Capture fps
    // * Video framerate
    // * is slow motion (detect by comparing capture fps and video fps)
    // * is timelapse



    // --- Construct and return the final struct ---
    TagData {
        is_video,
        capture_fps,
        video_fps,
        is_hdr,
        is_burst,
        burst_id,
        is_timelapse,
        is_slowmotion,
        is_photosphere,
        is_night_sight,
        is_motion_photo,
        projection_type,
        use_panorama_viewer,
        motion_photo_presentation_timestamp,
    }
}
