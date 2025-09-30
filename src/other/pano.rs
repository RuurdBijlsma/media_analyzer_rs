use crate::other::structs::{PanoInfo, PanoViewInfo};
use serde_json::Value;
use std::path::Path;

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
    use exiftool::ExifTool;
    use std::path::Path;

    #[test]
    fn test_photosphere() -> color_eyre::Result<()> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("photosphere.jpg");

        let mut et = ExifTool::new()?;
        let exif_data = et.json(&path, &["-n"])?;

        let pano_info = get_pano_info(&path, &exif_data);
        assert!(
            pano_info.is_photosphere,
            "Should be detected as a photosphere"
        );
        assert!(
            pano_info.use_panorama_viewer,
            "Should require a panorama viewer"
        );
        assert_eq!(
            pano_info.projection_type,
            Some("equirectangular".to_string()),
            "Projection type should be equirectangular"
        );
        assert!(pano_info.view_info.is_some());

        Ok(())
    }
}
