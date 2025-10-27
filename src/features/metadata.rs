use crate::features::error::MetadataError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileMetadata {
    pub width: u64,
    pub height: u64,
    pub mime_type: String,
    pub duration: Option<f64>,
    pub size_bytes: u64,
    pub orientation: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureDetails {
    pub iso: Option<u64>,
    pub exposure_time: Option<f64>,
    pub aperture: Option<f64>,
    pub focal_length: Option<f64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
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

pub fn get_metadata(exif: &Value) -> Result<(FileMetadata, CaptureDetails), MetadataError> {
    Ok((
        FileMetadata {
            width: get_required_u64(exif, "ImageWidth")?,
            height: get_required_u64(exif, "ImageHeight")?,
            mime_type: get_required_string(exif, "MIMEType")?,
            size_bytes: get_required_u64(exif, "FileSize")?,
            orientation: get_u64(exif, "Orientation"),
            duration: exif.get("Duration").and_then(parse_duration),
        },
        CaptureDetails {
            iso: get_u64(exif, "ISO"),
            exposure_time: get_f64(exif, "ExposureTime"),
            aperture: get_f64(exif, "Aperture"),
            focal_length: get_f64(exif, "FocalLengthIn35mmFormat")
                .or_else(|| get_f64(exif, "FocalLength")),
            camera_make: get_string(exif, "Make"),
            camera_model: get_string(exif, "Model"),
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
            "FileSize": 5242880, // 5 MB
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
        assert_eq!(metadata.size_bytes, 5242880);
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
        assert_eq!(capture_details.focal_length, Some(85.0));
    }

    #[test]
    fn test_get_metadata_with_minimal_video_data() {
        // Simulate minimal EXIF from a video file
        let exif_data = json!({
            "ImageWidth": 1920,
            "ImageHeight": 1080,
            "MIMEType": "video/mp4",
            "FileSize": 15728640, // 15 MB
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
        assert_eq!(metadata.size_bytes, 15728640);
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
            "ImageWidth": 1280, "ImageHeight": 720, "MIMEType": "video/webm", "FileSize": 1000000,
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
            "ImageWidth": 1280, "ImageHeight": 720, "MIMEType": "video/webm", "FileSize": 1000000,
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
        let mut et = ExifTool::new()?;
        let file = Path::new("assets/orientation-5.jpg");
        let numeric_exif = et.json(file, &["-n"])?;
        let (metadata, _) = get_metadata(&numeric_exif)?;

        assert_eq!(metadata.orientation, Some(5));

        Ok(())
    }

    #[test]
    fn test_focal_length_fallback_logic() {
        // Test that it correctly falls back to "FocalLength" if "FocalLengthIn35mmFormat" is missing.
        let exif_data = json!({
            "ImageWidth": 100, "ImageHeight": 100, "MIMEType": "image/jpeg", "FileSize": 1024,
            "FocalLength": 50.0 // The fallback field
        });
        let (_, capture_details) = get_metadata(&exif_data).unwrap();
        assert_eq!(capture_details.focal_length, Some(50.0));

        // Test that it prefers "FocalLengthIn35mmFormat" when both are present.
        let exif_data_prefer = json!({
            "ImageWidth": 100, "ImageHeight": 100, "MIMEType": "image/jpeg", "FileSize": 1024,
            "FocalLengthIn35mmFormat": 85.0, // The preferred field
            "FocalLength": 50.0
        });
        let (_, capture_details_prefer) = get_metadata(&exif_data_prefer).unwrap();
        assert_eq!(capture_details_prefer.focal_length, Some(85.0));
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
