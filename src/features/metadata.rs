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

pub fn get_metadata(exif_data: &Value) -> Result<(FileMetadata, CaptureDetails), MetadataError> {
    // Reusable helper closures
    let get_u64 = |key: &str| -> Result<u64, MetadataError> {
        exif_data
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| MetadataError::MissingRequiredField(key.to_string()))
    };

    let get_string = |key: &str| -> Result<String, MetadataError> {
        exif_data
            .get(key)
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or_else(|| MetadataError::MissingRequiredField(key.to_string()))
    };

    // --- These fields are required, so we use '?' ---
    let width = get_u64("ImageWidth")?;
    let height = get_u64("ImageHeight")?;
    let mime_type = get_string("MIMEType")?;
    let size_bytes = get_u64("FileSize")?; // Expecting file size is reasonable

    // --- Optional fields use .ok() ---
    // The Duration tag can be a number (seconds) or a string ("HH:MM:SS.fff").
    // We need to handle both cases.
    let duration = exif_data.get("Duration").and_then(|val| {
        // First, try to parse it as a number (f64).
        if let Some(d) = val.as_f64() {
            return Some(d);
        }
        // If that fails, try to parse it as a "HH:MM:SS.fff" string.
        if let Some(s) = val.as_str() {
            let parts: Vec<f64> = s.split(':').filter_map(|p| p.parse().ok()).collect();
            if parts.len() == 3 {
                return Some(parts[0] * 3600.0 + parts[1] * 60.0 + parts[2]);
            }
        }
        // If neither works, it's None.
        None
    });

    let iso = exif_data.get("ISO").and_then(Value::as_u64);
    // ... other optional fields ...
    let camera_make = exif_data
        .get("Make")
        .and_then(Value::as_str)
        .map(String::from);
    let camera_model = exif_data
        .get("Model")
        .and_then(Value::as_str)
        .map(String::from);
    let aperture = exif_data.get("Aperture").and_then(Value::as_f64);
    let exposure_time = exif_data.get("ExposureTime").and_then(Value::as_f64);
    let focal_length = exif_data
        .get("FocalLengthIn35mmFormat")
        .and_then(Value::as_f64)
        .or_else(|| exif_data.get("FocalLength").and_then(Value::as_f64));

    Ok((
        FileMetadata {
            width,
            height,
            mime_type,
            size_bytes,
            duration,
        },
        CaptureDetails {
            iso,
            exposure_time,
            aperture,
            focal_length,
            camera_make,
            camera_model,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::error::MetadataError;
    use serde_json::json;

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
