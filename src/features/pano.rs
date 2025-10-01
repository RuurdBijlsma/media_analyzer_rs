use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanoInfo {
    pub use_panorama_viewer: bool,
    pub is_photosphere: bool,
    pub view_info: Option<PanoViewInfo>,
    pub projection_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanoViewInfo {
    /// The calculated horizontal field of view in degrees.
    pub horizontal_fov_deg: f64,
    /// The calculated vertical field of view in degrees.
    pub vertical_fov_deg: f64,
    /// The horizontal center of the view in degrees (-180 to 180).
    pub center_yaw_deg: f64,
    /// The vertical center of the view in degrees (-90 to 90).
    pub center_pitch_deg: f64,
}

pub fn get_pano_info(file_path: &Path, exif: &Value) -> PanoInfo {
    let filename_lower = file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    let has_pano_in_filename = filename_lower.contains(".pano.");

    let projection_type = exif
        .get("XMP-GPano:ProjectionType")
        .or_else(|| exif.get("GPano:ProjectionType"))
        .or_else(|| exif.get("ProjectionType"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let is_equirectangular = projection_type
        .clone()
        .is_some_and(|s| s.eq_ignore_ascii_case("equirectangular"));

    // Step 2: If it's equirectangular, determine if it's a full sphere or a partial pano.
    let pano_info: Option<PanoViewInfo> = if is_equirectangular {
        // Attempt to parse the detailed GPano tags for a partial panorama.
        if let Some(partial_info) = parse_partial_pano_info(exif) {
            Some(partial_info)
        } else {
            // If the detailed tags are missing, assume it's a full 360° sphere.
            Some(PanoViewInfo {
                horizontal_fov_deg: 360.,
                vertical_fov_deg: 180.,
                center_yaw_deg: 0.,
                center_pitch_deg: 0.,
            })
        }
    } else {
        // Not an equirectangular projection, so not a spherical panorama.
        None
    };

    // If a projection type exists, it requires a panorama viewer.
    let use_panorama_viewer = pano_info.is_some() || has_pano_in_filename;

    // Step 3: Determine if the image should be treated as a full photosphere.
    let is_photosphere = if is_equirectangular {
        match &pano_info {
            Some(info) => {
                // Case A: We have explicit data. It's a photosphere only if the
                // data describes a full 360x180 degree view.
                let is_full_horizontal = (info.horizontal_fov_deg - 360.0).abs() < 0.1;
                let is_full_vertical = (info.vertical_fov_deg - 180.0).abs() < 0.1;
                is_full_horizontal && is_full_vertical
            }
            None => {
                // Case B: The image is equirectangular, but the detailed GPano tags
                // are missing. By convention, we must assume it's a full sphere.
                true
            }
        }
    } else {
        // Case C: Not an equirectangular image, so it cannot be a photosphere.
        false
    };

    PanoInfo {
        use_panorama_viewer,
        is_photosphere,
        projection_type,
        view_info: pano_info,
    }
}

/// Parses the GPano EXIF tags to calculate the dimensions of a partial panorama.
/// Returns None if the required tags are not present.
pub fn parse_partial_pano_info(exif: &Value) -> Option<PanoViewInfo> {
    // Attempt to get all six required values as f64. If any are missing, return None.
    let full_width = exif.get("FullPanoWidthPixels")?.as_f64()?;
    let full_height = exif.get("FullPanoHeightPixels")?.as_f64()?;
    let cropped_width = exif.get("CroppedAreaImageWidthPixels")?.as_f64()?;
    let cropped_height = exif.get("CroppedAreaImageHeightPixels")?.as_f64()?;
    let cropped_left = exif.get("CroppedAreaLeftPixels")?.as_f64()?;
    let cropped_top = exif.get("CroppedAreaTopPixels")?.as_f64()?;

    // Avoid division by zero.
    if full_width == 0.0 || full_height == 0.0 {
        return None;
    }

    // --- Calculate Field of View ---
    let horizontal_fov_deg = (cropped_width / full_width) * 360.0;
    let vertical_fov_deg = (cropped_height / full_height) * 180.0;

    // --- Calculate Center Point (Yaw and Pitch) ---
    // Yaw: Horizontal center. 0 is forward, -180 is left, 180 is right.
    let center_yaw_deg = ((cropped_left + cropped_width / 2.0) / full_width - 0.5) * 360.0;
    // Pitch: Vertical center. 0 is horizon, 90 is up, -90 is down.
    let center_pitch_deg = ((cropped_top + cropped_height / 2.0) / full_height - 0.5) * -180.0;

    Some(PanoViewInfo {
        horizontal_fov_deg,
        vertical_fov_deg,
        center_yaw_deg,
        center_pitch_deg,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;

    // --- Tests for the main `get_pano_info` function ---

    #[test]
    fn test_full_photosphere_from_exif() {
        // Simulates a standard photosphere with minimal EXIF, forcing the "default to full sphere" logic.
        let path = Path::new("photosphere.jpg");
        let exif_data = json!({
            "ProjectionType": "equirectangular"
        });

        let pano_info = get_pano_info(path, &exif_data);

        assert!(
            pano_info.is_photosphere,
            "Should be a photosphere by default"
        );
        assert!(pano_info.use_panorama_viewer, "Should use panorama viewer");
        assert_eq!(
            pano_info.projection_type,
            Some("equirectangular".to_string())
        );

        // Check that it correctly defaulted to a full 360x180 view
        let view_info = pano_info.view_info.unwrap();
        assert_eq!(view_info.horizontal_fov_deg, 360.0);
        assert_eq!(view_info.vertical_fov_deg, 180.0);
    }

    #[test]
    fn test_partial_equirectangular_pano_from_exif() {
        // Simulates a detailed partial panorama. This is the most complex case.
        let path = Path::new("partial_pano.jpg");
        let exif_data = json!({
            "ProjectionType": "equirectangular",
            "FullPanoWidthPixels": 8192,
            "FullPanoHeightPixels": 4096,
            "CroppedAreaImageWidthPixels": 4096, // Exactly half the width
            "CroppedAreaImageHeightPixels": 2048, // Exactly half the height
            "CroppedAreaLeftPixels": 2048, // Starts 1/4 of the way in
            "CroppedAreaTopPixels": 1024 // Starts 1/4 of the way down
        });

        let pano_info = get_pano_info(path, &exif_data);

        assert!(
            !pano_info.is_photosphere,
            "A partial pano should not be a photosphere"
        );
        assert!(
            pano_info.use_panorama_viewer,
            "Should still use a panorama viewer"
        );
        assert_eq!(
            pano_info.projection_type,
            Some("equirectangular".to_string())
        );

        let view_info = pano_info
            .view_info
            .expect("Should have view info for partial pano");

        // --- Verify the calculations from parse_partial_pano_info ---
        // Horizontal FOV should be (4096 / 8192) * 360 = 180 degrees
        assert!((view_info.horizontal_fov_deg - 180.0).abs() < 1e-9);
        // Vertical FOV should be (2048 / 4096) * 180 = 90 degrees
        assert!((view_info.vertical_fov_deg - 90.0).abs() < 1e-9);
        // Center Yaw should be ((2048 + 4096/2) / 8192 - 0.5) * 360 = 0 degrees (centered horizontally)
        assert!((view_info.center_yaw_deg - 0.0).abs() < 1e-9);
        // Center Pitch should be ((1024 + 2048/2) / 4096 - 0.5) * -180 = 0 degrees (centered vertically)
        assert!((view_info.center_pitch_deg - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_filename_triggers_viewer_without_exif() {
        // Test that the filename check works independently of EXIF data.
        let path = Path::new("some_image.pano.jpg");
        let exif_data = json!({}); // No pano EXIF tags

        let pano_info = get_pano_info(path, &exif_data);

        assert!(
            pano_info.use_panorama_viewer,
            "Filename '.pano.' should trigger viewer"
        );
        // Ensure other fields are correctly false/None
        assert!(!pano_info.is_photosphere);
        assert!(pano_info.projection_type.is_none());
        assert!(pano_info.view_info.is_none());
    }

    #[test]
    fn test_regular_image_is_not_a_pano() {
        // Test the most common case: a standard image.
        let path = Path::new("sunset.jpg");
        let exif_data = json!({}); // Empty EXIF

        let pano_info = get_pano_info(path, &exif_data);

        assert!(!pano_info.use_panorama_viewer);
        assert!(!pano_info.is_photosphere);
        assert!(pano_info.projection_type.is_none());
        assert!(pano_info.view_info.is_none());
    }

    // --- Tests specifically for the `parse_partial_pano_info` helper function ---

    #[test]
    fn test_parse_partial_fails_if_tag_is_missing() {
        // This JSON is missing "FullPanoHeightPixels"
        let incomplete_exif = json!({
            "FullPanoWidthPixels": 8192,
            "CroppedAreaImageWidthPixels": 4096,
            "CroppedAreaImageHeightPixels": 2048,
            "CroppedAreaLeftPixels": 2048,
            "CroppedAreaTopPixels": 1024
        });
        let result = parse_partial_pano_info(&incomplete_exif);
        assert!(
            result.is_none(),
            "Should fail gracefully if a required tag is missing"
        );
    }

    #[test]
    fn test_parse_partial_fails_on_division_by_zero() {
        let zero_width_exif = json!({
            "FullPanoWidthPixels": 0, // This would cause a division by zero
            "FullPanoHeightPixels": 4096,
            "CroppedAreaImageWidthPixels": 4096,
            "CroppedAreaImageHeightPixels": 2048,
            "CroppedAreaLeftPixels": 2048,
            "CroppedAreaTopPixels": 1024
        });
        let result = parse_partial_pano_info(&zero_width_exif);
        assert!(
            result.is_none(),
            "Should fail gracefully on potential division by zero"
        );
    }
}
