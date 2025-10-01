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
        && let Some(id) = caps.get(1)
    {
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
        && !id.is_empty()
    {
        return (true, Some(id));
    }

    // Layer 2: Fallback to filename-based detection for other devices (e.g., Samsung).
    detect_burst_from_filename(filename_lower)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- Tests for the orchestrator `find_burst_info` function ---

    #[test]
    fn test_find_burst_prefers_exif_burstuuid() {
        // Test that the Apple 'BurstUUID' is prioritized over other tags and the filename.
        let exif_data = json!({
            "BurstUUID": "APPLE-BURST-ID-123",
            "GCamera:BurstId": "GOOGLE-BURST-ID-456",
            "BurstId": "GENERIC-BURST-ID-789"
        });
        let filename = "some_burst_filename.jpg";

        let (is_burst, burst_id) = find_burst_info(&exif_data, filename);

        assert!(is_burst);
        assert_eq!(burst_id, Some("APPLE-BURST-ID-123".to_string()));
    }

    #[test]
    fn test_find_burst_uses_gcamera_burstid() {
        // Test that the Google 'GCamera:BurstId' is used when BurstUUID is absent.
        let exif_data = json!({
            "GCamera:BurstId": "GOOGLE-BURST-ID-456",
            "BurstId": "GENERIC-BURST-ID-789"
        });
        let filename = "some_burst_filename.jpg";

        let (is_burst, burst_id) = find_burst_info(&exif_data, filename);

        assert!(is_burst);
        assert_eq!(burst_id, Some("GOOGLE-BURST-ID-456".to_string()));
    }

    #[test]
    fn test_find_burst_uses_generic_burstid() {
        // Test that the generic 'BurstId' is used when others are absent.
        let exif_data = json!({
            "BurstId": "GENERIC-BURST-ID-789"
        });
        let filename = "some_burst_filename.jpg";

        let (is_burst, burst_id) = find_burst_info(&exif_data, filename);

        assert!(is_burst);
        assert_eq!(burst_id, Some("GENERIC-BURST-ID-789".to_string()));
    }

    #[test]
    fn test_find_burst_falls_back_to_filename() {
        // Test that when no EXIF tags are present, it correctly falls back to the filename.
        let exif_data = json!({}); // No burst tags
        let filename = "20150813_160421_burst01.jpg";

        let (is_burst, burst_id) = find_burst_info(&exif_data, filename);

        assert!(is_burst);
        assert_eq!(burst_id, Some("20150813_160421".to_string()));
    }

    #[test]
    fn test_find_burst_handles_empty_exif_tag() {
        // Test that if the EXIF tag is present but empty, it correctly falls back to the filename.
        let exif_data = json!({ "BurstUUID": "" });
        let filename = "google_burst_abc.jpg";

        let (is_burst, burst_id) = find_burst_info(&exif_data, filename);

        assert!(is_burst);
        assert_eq!(burst_id, Some("google".to_string()));
    }

    #[test]
    fn test_find_burst_returns_none_for_non_burst() {
        // Test the most common case: no burst information anywhere.
        let exif_data = json!({});
        let filename = "a_regular_photo.jpg";

        let (is_burst, burst_id) = find_burst_info(&exif_data, filename);

        assert!(!is_burst);
        assert!(burst_id.is_none());
    }

    // --- Unit tests for the helper `detect_burst_from_filename` function ---

    #[test]
    fn test_detect_from_filename_google_pixel_style() {
        let filename = "00000img_00000_burst20201123164411530_cover.jpg";
        let (is_burst, burst_id) = detect_burst_from_filename(filename);
        assert!(is_burst);
        assert_eq!(burst_id, Some("00000img_00000".to_string()));
    }

    #[test]
    fn test_detect_from_filename_samsung_style() {
        let filename = "20150813_160421_burst01.jpg";
        let (is_burst, burst_id) = detect_burst_from_filename(filename);
        assert!(is_burst);
        assert_eq!(burst_id, Some("20150813_160421".to_string()));
    }

    #[test]
    fn test_detect_from_filename_no_prefix() {
        // The regex requires a prefix before "_burst" to form an ID.
        let filename = "_burst_something.jpg";
        let (is_burst, burst_id) = detect_burst_from_filename(filename);
        assert!(
            !is_burst,
            "Should not be considered a burst if there is no ID prefix"
        );
        assert!(burst_id.is_none());
    }
}
