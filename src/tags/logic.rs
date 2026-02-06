use crate::tags::burst::find_burst_info;
use crate::tags::fps::get_fps;
use crate::tags::hdr::detect_hdr;
use crate::tags::structs::TagData;
use serde_json::Value;
use std::path::Path;

/// Extracts tags from a file's path and its EXIF metadata.
pub fn extract_tags(path: &Path, exif: &Value) -> TagData {
    let filename_lower = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    // --- Multi-layered Burst Detection ---
    let (is_burst, burst_id) = find_burst_info(exif, &filename_lower);

    // --- Other Tags from Filename ---
    let is_night_sight = filename_lower.contains("night");

    // --- Tags from EXIF data ---
    let is_motion_photo = exif
        .get("MotionPhoto")
        .and_then(Value::as_i64)
        .is_some_and(|x| x == 1);

    let motion_photo_presentation_timestamp = exif
        .get("MotionPhotoPresentationTimestampUs")
        .and_then(Value::as_i64);

    // --- Video Detection ---
    let is_video = exif
        .get("MIMEType")
        .and_then(|m| m.as_str())
        .is_some_and(|s| s.starts_with("video/"));

    let is_hdr = detect_hdr(exif);

    // --- Video Metadata ---
    let (video_fps, capture_fps) = get_fps(exif);

    // --- Slow Motion and Time-lapse ---
    let is_slowmotion = match (capture_fps, video_fps) {
        (Some(c_fps), Some(v_fps)) if v_fps > 0.0 => (c_fps / v_fps) > 1.05,
        _ => false,
    };

    let is_timelapse = exif
        .get("UserComment")
        .and_then(|c| c.as_str())
        .map_or_else(
            || {
                exif.get("Description")
                    .and_then(|d| d.as_str())
                    .map_or_else(
                        || {
                            // FIX APPLIED HERE: Replaced if let/else with map_or
                            exif.get("SpecialTypeID").and_then(|s| s.as_str()).map_or(
                                matches!(video_fps, Some(v_fps) if v_fps < 10.0),
                                |special_type| special_type.to_lowercase().contains("timelapse"),
                            )
                        },
                        |description| {
                            let desc = description.to_lowercase();
                            desc.contains("time-lapse") || desc.contains("hyperlapse")
                        },
                    )
            },
            |user_comment| {
                let comment = user_comment.to_lowercase();
                comment.contains("time-lapse") || comment.contains("hyperlapse")
            },
        );

    // --- Construct and return the final struct ---
    TagData {
        is_motion_photo,
        motion_photo_presentation_timestamp,
        is_night_sight,
        is_hdr,
        is_burst,
        burst_id,
        is_timelapse,
        is_slowmotion,
        is_video,
        capture_fps,
        video_fps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MediaAnalyzerError;
    use exiftool::ExifTool;
    use std::path::Path;

    /// Helper function to reduce boilerplate in tests.
    /// It takes a relative path to an asset, runs exiftool, and returns the extracted tags.
    fn get_tags_for_asset(relative_path: &str) -> Result<TagData, MediaAnalyzerError> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join(relative_path);

        assert!(path.exists(), "Test asset file not found at: {path:?}");

        // Use the numeric preset '-n' for all tests for consistency.
        let et = ExifTool::new()?;
        let exif_data = et.json(&path, &["-n"])?;

        Ok(extract_tags(&path, &exif_data))
    }

    #[test]
    fn test_night_sight_photo() {
        let tags = get_tags_for_asset("night_sight/PXL_20250104_170020532.NIGHT.jpg").unwrap();

        assert!(
            tags.is_night_sight,
            "Should be detected as Night Sight from filename"
        );

        // Ensure other boolean tags are false
        assert!(!tags.is_video);
        assert!(!tags.is_motion_photo);
        assert!(!tags.is_burst);
        assert!(!tags.is_slowmotion);
        assert!(!tags.is_timelapse);
    }

    #[test]
    fn test_motion_photo() {
        let tags = get_tags_for_asset("motion/PXL_20250103_180944831.MP.jpg").unwrap();

        assert!(
            tags.is_motion_photo,
            "Should be detected as a Motion Photo from EXIF tag"
        );
        assert!(
            tags.motion_photo_presentation_timestamp.is_some(),
            "Should have a presentation timestamp"
        );
        assert!(
            !tags.is_video,
            "Motion photos are not considered primary videos"
        );
    }

    #[test]
    fn test_hdr_photo() {
        let tags = get_tags_for_asset("hdr.jpg").unwrap();
        assert!(tags.is_hdr, "Should be detected as HDR from EXIF tag");
    }

    #[test]
    fn test_burst_photos() {
        // Google Pixel burst format
        let tags1 =
            get_tags_for_asset("burst/00000IMG_00000_BURST20201123164411530_COVER.jpg").unwrap();
        assert!(tags1.is_burst, "Should detect Google burst format");
        assert_eq!(tags1.burst_id, Some("00000img_00000".to_string()));

        // Samsung/Older burst format
        let tags2 = get_tags_for_asset("burst/20150813_160421_Burst01.jpg").unwrap();
        assert!(tags2.is_burst, "Should detect Samsung burst format");
        assert_eq!(tags2.burst_id, Some("20150813_160421".to_string()));
    }

    #[test]
    fn test_slow_motion_video() {
        let tags = get_tags_for_asset("slowmotion.mp4").unwrap();

        assert!(tags.is_video);
        assert!(tags.is_slowmotion);
        assert!(!tags.is_timelapse);

        let capture_fps = tags.capture_fps.expect("Should have capture FPS");
        let video_fps = tags.video_fps.expect("Should have video FPS");

        assert!(
            capture_fps > video_fps,
            "Capture FPS must be greater than video FPS"
        );
        assert!(
            (capture_fps / video_fps) > 1.05,
            "Slow motion ratio should be > 1.05"
        );
    }

    #[test]
    fn test_timelapse_video() {
        let tags = get_tags_for_asset("timelapse.mp4").unwrap();

        assert!(tags.is_video);
        assert!(tags.is_timelapse);
        assert!(!tags.is_slowmotion);

        // Verify the fallback detection logic for timelapse (low video FPS)
        tags.video_fps.expect("Timelapse should have video FPS");
    }

    #[test]
    fn test_standard_video() {
        let tags = get_tags_for_asset("video/car.webm").unwrap();

        assert!(tags.is_video);

        // Ensure other boolean tags are false
        assert!(!tags.is_slowmotion);
        assert!(!tags.is_timelapse);
        assert!(!tags.is_motion_photo);
        assert!(!tags.is_burst);
        assert!(!tags.is_night_sight);
    }

    #[test]
    fn test_standard_image_properties() {
        let tags = get_tags_for_asset("tent.jpg").unwrap();

        // Assert all boolean flags are correctly false for a standard image
        assert!(!tags.is_video);
        assert!(!tags.is_burst);
        assert!(!tags.is_night_sight);
        assert!(!tags.is_motion_photo);
        assert!(!tags.is_slowmotion);
        assert!(!tags.is_timelapse);

        // Assert all optional fields are None
        assert!(tags.burst_id.is_none());
        assert!(tags.motion_photo_presentation_timestamp.is_none());
    }

    #[test]
    fn test_non_media_file() {
        // This file type won't have any media EXIF tags
        let tags = get_tags_for_asset("text_file.txt").unwrap();

        // Assert that all boolean flags are false
        assert!(!tags.is_video);
        assert!(!tags.is_burst);
        assert!(!tags.is_hdr);
        assert!(!tags.is_motion_photo);
        assert!(!tags.is_night_sight);
        assert!(!tags.is_slowmotion);
        assert!(!tags.is_timelapse);

        // Assert that all optional fields are None
        assert!(tags.burst_id.is_none());
        assert!(tags.capture_fps.is_none());
        assert!(tags.video_fps.is_none());
        assert!(tags.motion_photo_presentation_timestamp.is_none());
    }
}
