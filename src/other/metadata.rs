use crate::other::structs::MediaMetadata;
use color_eyre::eyre::{eyre, Context};
use serde_json::Value;

/// Extracts media metadata from a serde_json::Value object for both images and videos.
pub fn get_metadata(exif_data: &Value) -> color_eyre::Result<MediaMetadata> {
    // --- Reusable helper closures for safe JSON parsing ---

    let get_u64 = |key: &str| -> Result<u64, color_eyre::eyre::Error> {
        exif_data
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| eyre!("Missing or invalid u64 for key: '{}'", key))
    };

    let get_f64 = |key: &str| -> Result<f64, color_eyre::eyre::Error> {
        exif_data
            .get(key)
            .and_then(Value::as_f64)
            .ok_or_else(|| eyre!("Missing or invalid f64 for key: '{}'", key))
    };

    let get_string = |key: &str| -> Result<String, color_eyre::eyre::Error> {
        exif_data
            .get(key)
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or_else(|| eyre!("Missing or invalid string for key: '{}'", key))
    };

    // --- Parse common metadata fields that should exist for all media ---

    let width = get_u64("ImageWidth").context("Failed to parse 'ImageWidth'")?;
    let height = get_u64("ImageHeight").context("Failed to parse 'ImageHeight'")?;
    let mime_type = get_string("MIMEType").context("Failed to parse 'MIMEType'")?;

    // --- Parse optional fields ---

    let size_bytes = get_u64("FileSize").ok();
    let duration = get_f64("Duration").ok();
    // assume ISO is an integer, common in images.
    let iso = get_u64("ISO").ok();
    // assume ExposureTime is a float in seconds.
    let exposure_time = get_f64("ExposureTime").ok();
    // Get camera make and model
    let camera_make = get_string("Make").ok();
    let camera_model = get_string("Model").ok();


    Ok(MediaMetadata {
        width,
        height,
        duration,
        size_bytes,
        mime_type,
        iso,
        exposure_time,
        camera_make,
        camera_model,
    })
}