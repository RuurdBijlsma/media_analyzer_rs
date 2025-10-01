use serde_json::Value;

pub fn detect_hdr(v: &Value) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_detects_hdr_from_composite_image() {
        let exif = json!({ "CompositeImage": 3 });
        assert!(
            detect_hdr(&exif),
            "Should detect HDR when CompositeImage is 3"
        );

        let exif_not_hdr = json!({ "CompositeImage": 2 });
        assert!(
            !detect_hdr(&exif_not_hdr),
            "Should not detect HDR for other CompositeImage values"
        );
    }

    #[test]
    fn test_detects_hdr_from_scene_capture_type() {
        let exif = json!({ "SceneCaptureType": 3 });
        assert!(
            detect_hdr(&exif),
            "Should detect HDR when SceneCaptureType is 3 (HDR)"
        );

        let exif_not_hdr = json!({ "SceneCaptureType": 1 }); // Standard
        assert!(
            !detect_hdr(&exif_not_hdr),
            "Should not detect HDR for other SceneCaptureType values"
        );
    }

    #[test]
    fn test_detects_hdr_from_hdrimagetype_tag_presence() {
        // The presence of the tag, regardless of its value, should trigger detection.
        let exif = json!({ "HDRImageType": "HDR" });
        assert!(
            detect_hdr(&exif),
            "Should detect HDR if HDRImageType tag exists"
        );
    }

    #[test]
    fn test_detects_hdr_from_software_string() {
        let exif_lower = json!({ "Software": "Shot on Pixel with hdr+" });
        assert!(
            detect_hdr(&exif_lower),
            "Should detect HDR from lowercase 'hdr' in Software tag"
        );

        let exif_upper = json!({ "Software": "ACME HDR Pro" });
        assert!(
            detect_hdr(&exif_upper),
            "Should detect HDR from uppercase 'HDR' in Software tag"
        );

        let exif_not_hdr = json!({ "Software": "Adobe Photoshop" });
        assert!(
            !detect_hdr(&exif_not_hdr),
            "Should not detect HDR if 'hdr' is not in Software tag"
        );
    }

    #[test]
    fn test_detects_hdr_from_gainmapimage_tag() {
        let exif = json!({ "GainMapImage": "some_data_here" });
        assert!(
            detect_hdr(&exif),
            "Should detect HDR from presence of GainMapImage tag"
        );
    }

    #[test]
    fn test_detects_hdr_from_directoryitemsemantic_array() {
        let exif_correct_case = json!({
            "DirectoryItemSemantic": ["Image", "GainMap"]
        });
        assert!(
            detect_hdr(&exif_correct_case),
            "Should detect HDR from 'GainMap' in array"
        );

        let exif_wrong_case = json!({
            "DirectoryItemSemantic": ["image", "gainmap"]
        });
        assert!(
            detect_hdr(&exif_wrong_case),
            "Should detect HDR from 'gainmap' in array (case-insensitive)"
        );

        let exif_not_hdr = json!({
            "DirectoryItemSemantic": ["Image", "Primary"]
        });
        assert!(
            !detect_hdr(&exif_not_hdr),
            "Should not detect HDR if 'GainMap' is not in the array"
        );

        let exif_not_array = json!({ "DirectoryItemSemantic": "NotAnArray" });
        assert!(
            !detect_hdr(&exif_not_array),
            "Should not panic if DirectoryItemSemantic is not an array"
        );
    }

    #[test]
    fn test_returns_false_for_standard_image_exif() {
        let exif = json!({
            "ImageWidth": 4000,
            "ImageHeight": 3000,
            "Software": "Adobe Photoshop"
        });
        assert!(
            !detect_hdr(&exif),
            "Should return false for a typical non-HDR image"
        );
    }

    #[test]
    fn test_returns_false_for_empty_exif() {
        let exif = json!({});
        assert!(
            !detect_hdr(&exif),
            "Should return false for an empty EXIF object"
        );
    }
}
