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
fn detect_burst(filename_lower: &str) -> (bool, Option<String>) {
    // The initial check is a good, fast way to exit if "burst" isn't in the name.
    if !filename_lower.contains("burst") {
        return (false, None);
    }

    lazy_static! {
        // This single, robust pattern captures the common filename prefix before "_burst".
        // It works for both:
        // - "20150813_160421_burst01.jpg" -> ID: "20150813_160421"
        // - "00000img_00000_burst20201123164411530_cover.jpg" -> ID: "00000img_00000"
        static ref BURST_ID_PATTERN: Regex = Regex::new(r"(?i)(.*?)_burst.*").unwrap();
    }

    if let Some(caps) = BURST_ID_PATTERN.captures(filename_lower) {
        // We capture the first group (.*?), which is the prefix.
        if let Some(id) = caps.get(1) {
            let burst_id = id.as_str();
            // Ensure the captured ID is not empty (e.g., for a filename like "_burst.jpg")
            if !burst_id.is_empty() {
                return (true, Some(burst_id.to_string()));
            }
        }
    }

    // Return false if no valid ID could be extracted.
    (false, None)
}

fn detect_hdr(v: &Value) -> bool {
    // 1. Pixel: CompositeImage == 3
    if v.get("CompositeImage")
        .and_then(|x| x.as_i64())
        .map(|x| x == 3)
        .unwrap_or(false)
    {
        return true;
    }

    // 2. SceneCaptureType == 3 (some DSLRs / iPhones)
    if v.get("SceneCaptureType")
        .and_then(|x| x.as_i64())
        .map(|x| x == 3)
        .unwrap_or(false)
    {
        return true;
    }

    // 3. Explicit HDR tag
    if v.get("HDRImageType").is_some() {
        return true;
    }

    // 4. Software string contains "hdr"
    if v.get("Software")
        .and_then(|x| x.as_str())
        .map(|s| s.to_lowercase().contains("hdr"))
        .unwrap_or(false)
    {
        return true;
    }

    // 5. XMP / gain map detection
    if v.get("GainMapImage").is_some()
        || v.get("DirectoryItemSemantic")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter().any(|s| {
                    s.as_str()
                        .map(|s| s.eq_ignore_ascii_case("GainMap"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    {
        return true;
    }

    false
}

/// Parses frame rate values which can be integers, floats, or fractions (e.g., "30000/1001").
fn parse_fps(value: &Value) -> Option<f64> {
    if let Some(fps_str) = value.as_str() {
        if fps_str.contains('/') {
            let parts: Vec<&str> = fps_str.split('/').collect();
            if parts.len() == 2
                && let (Ok(num), Ok(den)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>())
                && den != 0.0
            {
                return Some(num / den);
            }
        } else if let Ok(fps) = fps_str.parse::<f64>() {
            return Some(fps);
        }
    } else if let Some(fps) = value.as_f64() {
        return Some(fps);
    }
    None
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
    let has_pano_in_filename = filename_lower.contains(".pano.");

    // --- Tags from EXIF data ---

    let is_motion_photo = exif
        .get("MotionPhoto")
        .and_then(|x| x.as_i64())
        .map(|x| x == 1)
        .unwrap_or(false);

    let motion_photo_presentation_timestamp = exif
        .get("MotionPhotoPresentationTimestampUs")
        .and_then(|x| x.as_i64());

    // --- Video Detection ---
    let is_video = exif
        .get("MIMEType")
        .and_then(|m| m.as_str())
        .map(|s| s.starts_with("video/"))
        .unwrap_or(false);

    let is_hdr = detect_hdr(exif);

    // --- Panorama and Photosphere Detection ---
    let projection_type = exif
        .get("XMP-GPano:ProjectionType")
        .or_else(|| exif.get("GPano:ProjectionType"))
        .or_else(|| exif.get("ProjectionType")) // <-- Added to find the key from your PANO sample
        .and_then(|v| v.as_str())
        .map(String::from);

    // If a projection type exists, it requires a panorama viewer. This is more reliable.
    let use_panorama_viewer = projection_type.is_some() || has_pano_in_filename;

    let is_photosphere = projection_type
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case("equirectangular"));

    // --- Video Metadata ---
    let video_fps = exif
        .get("AvgFrameRate")
        .or_else(|| exif.get("FrameRate"))
        .or_else(|| exif.get("VideoFrameRate"))
        .and_then(parse_fps);

    // Capture FPS is often the same as video FPS unless it's slow motion.
    // Some devices might have a specific tag, but this is a reliable default.
    let capture_fps = exif
        .get("SourceFrameRate") // A potential tag for capture FPS
        .or_else(|| exif.get("AndroidCaptureFPS"))
        .and_then(parse_fps)
        .or(video_fps);

    // --- Slow Motion and Time-lapse ---
    let is_slowmotion = match (capture_fps, video_fps) {
        (Some(c_fps), Some(v_fps)) if v_fps > 0.0 => (c_fps / v_fps) > 1.05,
        _ => false,
    };

    let is_timelapse = if let Some(user_comment) = exif.get("UserComment").and_then(|c| c.as_str())
    {
        let comment = user_comment.to_lowercase();
        comment.contains("time-lapse") || comment.contains("hyperlapse")
    } else if let Some(description) = exif.get("Description").and_then(|d| d.as_str()) {
        let desc = description.to_lowercase();
        desc.contains("time-lapse") || desc.contains("hyperlapse")
    } else if let Some(special_type) = exif.get("SpecialTypeID").and_then(|s| s.as_str()) {
        // This is a specific tag used by Google Pixel for special video types.
        special_type.to_lowercase().contains("timelapse")
    } else {
        // Fallback for other devices: very low FPS is a strong indicator.
        matches!(video_fps, Some(v_fps) if v_fps < 10.0)
    };

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
