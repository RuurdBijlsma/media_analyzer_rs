use lazy_static::lazy_static;
use regex::Regex;
use serde_json::Value;

/// Layer 2 of burst detection: Detects burst photos from filename conventions (primarily Android).
pub fn detect_burst_from_filename(filename_lower: &str) -> (bool, Option<String>) {
    if !filename_lower.contains("burst") {
        return (false, None);
    }

    lazy_static! {
        // Captures the common filename prefix before "_burst".
        static ref BURST_ID_PATTERN: Regex = Regex::new(r"(?i)(.*?)_burst.*").unwrap();
    }

    if let Some(caps) = BURST_ID_PATTERN.captures(filename_lower)
        && let Some(id) = caps.get(1) {
        let burst_id = id.as_str();
        if !burst_id.is_empty() {
            return (true, Some(burst_id.to_string()));
        }
    }
    (false, None)
}

/// Orchestrates burst detection using a multi-layered approach for maximum compatibility.
pub fn find_burst_info(exif: &Value, filename_lower: &str) -> (bool, Option<String>) {
    // Layer 1: Check for explicit EXIF burst tags (most reliable method).
    // - BurstUUID is the standard for Apple devices.
    // - GCamera:BurstId is a specific XMP tag used by Google Camera.
    let exif_burst_id = exif
        .get("BurstUUID")
        .or_else(|| exif.get("GCamera:BurstId"))
        .or_else(|| exif.get("BurstId"))
        .and_then(|v| v.as_str().map(String::from));

    if let Some(id) = exif_burst_id
        && !id.is_empty() {
        return (true, Some(id));
    }

    // Layer 2: Fallback to filename-based detection for other devices (e.g., Samsung).
    detect_burst_from_filename(filename_lower)
}