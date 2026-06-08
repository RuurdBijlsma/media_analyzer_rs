use crate::features::error::MetadataError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::mem;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BasicMetadata {
    pub width: u64,
    pub height: u64,
    pub mime_type: String,
    pub duration: Option<f64>,
    pub size_bytes: u64,
    pub orientation: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FlashMode {
    Unknown,
    CompulsoryFiring,
    CompulsorySuppression,
    Auto,
}

impl FlashMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::CompulsoryFiring => "CompulsoryFiring",
            Self::CompulsorySuppression => "CompulsorySuppression",
            Self::Auto => "Auto",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FlashInfo {
    pub fired: bool,
    pub mode: FlashMode,
    pub return_detected: Option<bool>,
    pub red_eye_reduction: bool,
    pub flash_function_present: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CameraSettings {
    pub iso: Option<u64>,
    pub exposure_time: Option<f64>,
    pub aperture: Option<f64>,
    pub focal_length: Option<f64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub focal_length_in_35mm: Option<f64>,
    pub lens_make: Option<String>,
    pub lens_model: Option<String>,
    pub flash: Option<FlashInfo>,
    pub digital_zoom_ratio: Option<f64>,
    pub subject_distance: Option<f64>,
    pub exposure_compensation: Option<f64>,
}

fn get_required_u64(exif: &Value, key: &str) -> Result<u64, MetadataError> {
    exif.get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| MetadataError::MissingRequiredField(key.to_string()))
}

fn get_required_string(exif: &Value, key: &str) -> Result<String, MetadataError> {
    exif.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| MetadataError::MissingRequiredField(key.to_string()))
}

fn get_f64(exif: &Value, key: &str) -> Option<f64> {
    exif.get(key).and_then(Value::as_f64)
}

fn get_u64(exif: &Value, key: &str) -> Option<u64> {
    exif.get(key).and_then(Value::as_u64)
}

fn get_string(exif: &Value, key: &str) -> Option<String> {
    exif.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn parse_duration(val: &Value) -> Option<f64> {
    if let Some(d) = val.as_f64() {
        return Some(d);
    }

    val.as_str().and_then(|s| {
        let parts: Vec<f64> = s.split(':').filter_map(|p| p.parse().ok()).collect();
        (parts.len() == 3).then(|| parts[0].mul_add(3600.0, parts[1] * 60.0) + parts[2])
    })
}

const fn parse_flash(raw: u64) -> FlashInfo {
    let fired = raw & 0x1 != 0;
    let return_bits = (raw >> 1) & 0x3;
    let mode_bits = (raw >> 3) & 0x3;
    let no_flash_function = raw & (1 << 5) != 0;
    let red_eye = raw & (1 << 6) != 0;

    let mode = match mode_bits {
        0 => FlashMode::CompulsorySuppression,
        1 => FlashMode::CompulsoryFiring,
        2 => FlashMode::Auto,
        _ => FlashMode::Unknown,
    };

    let return_detected = match return_bits {
        2 => Some(true),
        3 => Some(false),
        _ => None,
    };

    FlashInfo {
        fired,
        mode,
        return_detected,
        red_eye_reduction: red_eye,
        flash_function_present: !no_flash_function,
    }
}

pub fn get_metadata(exif: &Value) -> Result<(BasicMetadata, CameraSettings), MetadataError> {
    let mut width = get_required_u64(exif, "ImageWidth")?;
    let mut height = get_required_u64(exif, "ImageHeight")?;
    let orientation = get_u64(exif, "Orientation");
    let is_video_rotated = get_u64(exif, "Rotation").is_some_and(|r| r == 90 || r == 270);
    let is_photo_rotated = orientation.is_some_and(|o| (5..=8).contains(&o));
    if is_photo_rotated || is_video_rotated {
        // Swap width and height for 90 and 270-degree rotations
        mem::swap(&mut width, &mut height);
    }
    Ok((
        BasicMetadata {
            width,
            height,
            mime_type: get_required_string(exif, "MIMEType")?,
            size_bytes: get_required_u64(exif, "FileSize")?,
            orientation,
            duration: exif.get("Duration").and_then(parse_duration),
        },
        CameraSettings {
            iso: get_u64(exif, "ISO"),
            exposure_time: get_f64(exif, "ExposureTime"),
            aperture: get_f64(exif, "FNumber")
                .or_else(|| get_f64(exif, "Aperture"))
                .or_else(|| get_f64(exif, "ApertureValue")),
            focal_length: get_f64(exif, "FocalLength"),
            focal_length_in_35mm: get_f64(exif, "FocalLengthIn35mmFormat"),
            camera_make: get_string(exif, "Make").or_else(|| get_string(exif, "AndroidMake")),
            camera_model: get_string(exif, "Model").or_else(|| get_string(exif, "AndroidModel")),
            lens_make: get_string(exif, "LensMake"),
            lens_model: get_string(exif, "LensModel"),
            flash: exif.get("Flash").and_then(|v| v.as_u64().map(parse_flash)),
            digital_zoom_ratio: get_f64(exif, "DigitalZoomRatio"),
            subject_distance: get_f64(exif, "SubjectDistance"),
            exposure_compensation: get_f64(exif, "ExposureCompensation")
                .or_else(|| get_f64(exif, "ExposureBiasValue")),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MediaAnalyzerError;
    use crate::features::error::MetadataError;
    use exiftool::ExifTool;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn test_get_metadata_with_full_photo_data() {
        // Simulate a rich EXIF block from a photo
        let exif_data = json!({
            "ImageWidth": 4000,
            "ImageHeight": 3000,
            "MIMEType": "image/jpeg",
            "FileSize": 5_242_880, // 5 MB
            "ISO": 100,
            "Make": "Canon",
            "Model": "Canon EOS R5",
            "Aperture": 1.8,
            "ExposureTime": 0.004, // 1/250s
            "FocalLengthIn35mmFormat": 85.0
        });

        let result = get_metadata(&exif_data);
        assert!(result.is_ok(), "Should successfully parse full EXIF data");
        let (metadata, capture_details) = result.unwrap();

        // --- Assert FileMetadata ---
        assert_eq!(metadata.width, 4000);
        assert_eq!(metadata.height, 3000);
        assert_eq!(metadata.mime_type, "image/jpeg");
        assert_eq!(metadata.size_bytes, 5_242_880);
        assert!(metadata.duration.is_none());

        // --- Assert CaptureDetails ---
        assert_eq!(capture_details.iso, Some(100));
        assert_eq!(capture_details.camera_make, Some("Canon".to_string()));
        assert_eq!(
            capture_details.camera_model,
            Some("Canon EOS R5".to_string())
        );
        assert_eq!(capture_details.aperture, Some(1.8));
        assert_eq!(capture_details.exposure_time, Some(0.004));
        assert_eq!(capture_details.focal_length_in_35mm, Some(85.0));
        assert_eq!(capture_details.focal_length, None);
    }

    #[test]
    fn test_get_metadata_with_minimal_video_data() {
        // Simulate minimal EXIF from a video file
        let exif_data = json!({
            "ImageWidth": 1920,
            "ImageHeight": 1080,
            "MIMEType": "video/mp4",
            "FileSize": 15_728_640, // 15 MB
            "Duration": 10.53
        });

        let result = get_metadata(&exif_data);
        assert!(
            result.is_ok(),
            "Should successfully parse minimal video data"
        );
        let (metadata, capture_details) = result.unwrap();

        // --- Assert FileMetadata ---
        assert_eq!(metadata.width, 1920);
        assert_eq!(metadata.height, 1080);
        assert_eq!(metadata.mime_type, "video/mp4");
        assert_eq!(metadata.size_bytes, 15_728_640);
        assert_eq!(metadata.duration, Some(10.53));

        // --- Assert CaptureDetails ---
        // All optional photo-specific fields should be None
        assert!(capture_details.iso.is_none());
        assert!(capture_details.camera_make.is_none());
        assert!(capture_details.camera_model.is_none());
        assert!(capture_details.aperture.is_none());
        assert!(capture_details.exposure_time.is_none());
        assert!(capture_details.focal_length.is_none());
    }

    #[test]
    fn test_get_metadata_with_string_duration() {
        // Verifies the new case (e.g., from WebM files)
        let exif_data = json!({
            "ImageWidth": 1280, "ImageHeight": 720, "MIMEType": "video/webm", "FileSize": 1_000_000,
            "Duration": "00:00:05.874000000"
        });
        let (metadata, _) = get_metadata(&exif_data).unwrap();
        assert!(
            metadata.duration.is_some(),
            "Duration should be parsed from string"
        );
        // Use an epsilon for float comparison
        assert!((metadata.duration.unwrap() - 5.874).abs() < 1e-9);
    }

    #[test]
    fn test_get_metadata_handles_malformed_string_duration() {
        // Ensures that a bad string format doesn't cause a panic.
        let exif_data = json!({
            "ImageWidth": 1280, "ImageHeight": 720, "MIMEType": "video/webm", "FileSize": 1_000_000,
            "Duration": "5 seconds"
        });
        let (metadata, _) = get_metadata(&exif_data).unwrap();
        assert!(
            metadata.duration.is_none(),
            "Malformed duration string should result in None"
        );
    }

    #[test]
    fn test_orientation_tag() -> Result<(), MediaAnalyzerError> {
        let et = ExifTool::new()?;
        let file = Path::new("assets/orientation-5.jpg");
        let numeric_exif = et.json(file, &["-n"])?;
        let (metadata, _) = get_metadata(&numeric_exif)?;

        assert_eq!(metadata.orientation, Some(5));
        assert_eq!(metadata.width, 1800);
        assert_eq!(metadata.height, 1200);

        Ok(())
    }

    #[test]
    fn test_video_rotation_tag() -> Result<(), MediaAnalyzerError> {
        let et = ExifTool::new()?;
        let file = Path::new("assets/video/get_rotated_idiot.mp4");
        let numeric_exif = et.json(file, &["-n"])?;
        let (metadata, _) = get_metadata(&numeric_exif)?;

        assert_eq!(metadata.width, 1080);
        assert_eq!(metadata.height, 1920);

        Ok(())
    }

    #[test]
    fn test_focal_length_fallback_logic() {
        // Test that it correctly falls back to "FocalLength" if "FocalLengthIn35mmFormat" is missing.
        let exif_data = json!({
            "ImageWidth": 100, "ImageHeight": 100, "MIMEType": "image/jpeg", "FileSize": 1024,
            "FocalLengthIn35mmFormat": 85.0,
            "FocalLength": 50.0
        });
        let (_, capture_details) = get_metadata(&exif_data).unwrap();
        assert_eq!(capture_details.focal_length, Some(50.0));
        assert_eq!(capture_details.focal_length_in_35mm, Some(85.0));
    }

    #[test]
    fn test_fails_when_required_field_is_missing() {
        // Test case for missing "ImageWidth"
        let missing_width = json!({
            "ImageHeight": 100, "MIMEType": "image/jpeg", "FileSize": 1024
        });
        let result_width = get_metadata(&missing_width);
        assert!(
            matches!(result_width.unwrap_err(), MetadataError::MissingRequiredField(field) if field == "ImageWidth"),
            "Should fail with specific error for missing ImageWidth"
        );

        // Test case for missing "MIMEType"
        let missing_mime = json!({
            "ImageWidth": 100, "ImageHeight": 100, "FileSize": 1024
        });
        let result_mime = get_metadata(&missing_mime);
        assert!(
            matches!(result_mime.unwrap_err(), MetadataError::MissingRequiredField(field) if field == "MIMEType"),
            "Should fail with specific error for missing MIMEType"
        );
    }
}
