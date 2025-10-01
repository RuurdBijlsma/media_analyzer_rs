use crate::features::error::MetadataError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct FileMetadata {
    pub width: u64,
    pub height: u64,
    pub mime_type: String,
    pub duration: Option<f64>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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
    let duration = exif_data.get("Duration").and_then(Value::as_f64);
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
