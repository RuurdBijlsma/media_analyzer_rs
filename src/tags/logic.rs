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

    // --- Video Metadata ---
    let (video_fps, capture_fps) = get_fps(exif);

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
        special_type.to_lowercase().contains("timelapse")
    } else {
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
        is_night_sight,
        is_motion_photo,
        motion_photo_presentation_timestamp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exiftool::ExifTool;
    use std::path::Path;

    /// Helper function to reduce boilerplate in tests.
    /// It takes a relative path to an asset, runs exiftool, and returns the extracted tags.
    fn get_tags_for_asset(relative_path: &str) -> color_eyre::Result<TagData> {
        // Assume tests run from the project root where the 'assets' dir is.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join(relative_path);

        if !path.exists() {
            panic!("Test asset file not found at: {:?}", path);
        }

        let mut et = ExifTool::new()?;
        let exif_data = et.json(&path, &["-n"])?;

        // println!("{}", serde_json::to_string_pretty(&exif_data).unwrap());

        Ok(extract_tags(&path, &exif_data))
    }

    #[test]
    fn test_night_sight_photo() {
        let tags = get_tags_for_asset("night_sight/PXL_20250104_170020532.NIGHT.jpg").unwrap();
        assert!(
            tags.is_night_sight,
            "Should be detected as Night Sight from filename"
        );
        assert!(!tags.is_video);
        assert!(!tags.is_motion_photo);
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
        // The primary file is an image, but it contains a video component.
        assert!(
            !tags.is_video,
            "Motion photos contain a video stream but is not a video."
        );
    }

    #[test]
    fn test_burst_photos() {
        // Test case 1: Google Pixel burst format
        let tags1 =
            get_tags_for_asset("burst/00000IMG_00000_BURST20201123164411530_COVER.jpg").unwrap();
        assert!(tags1.is_burst, "Should detect burst format 1");
        assert_eq!(
            tags1.burst_id,
            Some("00000img_00000".to_string()),
            "Should extract correct burst ID for format 1"
        );

        // Test case 2: Samsung/Older burst format
        let tags2 = get_tags_for_asset("burst/20150813_160421_Burst01.jpg").unwrap();
        assert!(tags2.is_burst, "Should detect burst format 2");
        assert_eq!(
            tags2.burst_id,
            Some("20150813_160421".to_string()),
            "Should extract correct burst ID for format 2"
        );
    }

    #[test]
    fn test_slow_motion_video() {
        let tags = get_tags_for_asset("slowmotion.mp4").unwrap();
        assert!(tags.is_video, "Should be detected as a video");
        assert!(tags.is_slowmotion, "Should be detected as slow motion");
        assert!(!tags.is_timelapse, "Should not be a timelapse");

        // This assertion is key for slow motion detection
        if let (Some(capture), Some(video)) = (tags.capture_fps, tags.video_fps) {
            assert!(
                capture > video,
                "Capture FPS ({}) must be greater than video FPS ({}) for slow motion",
                capture,
                video
            );
        } else {
            panic!("Capture FPS and Video FPS could not be determined for slow motion file.");
        }
    }

    #[test]
    fn test_timelapse_video() {
        let tags = get_tags_for_asset("timelapse.mp4").unwrap();
        assert!(tags.is_video, "Should be detected as a video");
        assert!(tags.is_timelapse, "Should be detected as a timelapse");
        assert!(!tags.is_slowmotion, "Should not be slow motion");
    }

    #[test]
    fn test_standard_video() {
        let tags = get_tags_for_asset("video/car.webm").unwrap();
        assert!(tags.is_video, "Should be detected as a video");
        assert!(!tags.is_slowmotion);
        assert!(!tags.is_timelapse);
        assert!(!tags.is_motion_photo);
    }

    #[test]
    fn test_standard_images() {
        let tags_jpg = get_tags_for_asset("sunset.jpg").unwrap();
        assert!(!tags_jpg.is_video, "Standard JPG should not be a video");
        assert!(
            !tags_jpg.is_burst,
            "Standard JPG should not be a burst photo"
        );
        assert!(
            !tags_jpg.is_night_sight,
            "Standard JPG should not be night sight"
        );

        let tags_png = get_tags_for_asset("png_image.png").unwrap();
        assert!(!tags_png.is_video, "PNG should not be a video");

        let tags_gif = get_tags_for_asset("cat_bee.gif").unwrap();
        assert!(!tags_gif.is_video, "GIF should not be a video");
    }

    #[test]
    fn test_non_media_file() {
        // This will get an empty JSON object from our robust helper function
        let tags = get_tags_for_asset("text_file.txt").unwrap();

        // Assert that all boolean flags are false and Options are None
        assert!(!tags.is_video);
        assert!(!tags.is_burst);
        assert!(!tags.is_hdr);
        assert!(!tags.is_motion_photo);
        assert!(!tags.is_night_sight);
        assert!(!tags.is_slowmotion);
        assert!(!tags.is_timelapse);
        assert!(tags.burst_id.is_none());
        assert!(tags.capture_fps.is_none());
        assert!(tags.video_fps.is_none());
    }
}
