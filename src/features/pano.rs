use crate::ExifData;

pub fn should_use_pano_viewer(exif: &ExifData) -> bool {
    if let Some(use_panorama_viewer) = exif.get_bool_ignoring_case("UsePanoramaViewer") {
        return use_panorama_viewer;
    }

    // 3. Extract dimensions and projection metadata
    let cropped_width = exif.get_f64_ignoring_case("CroppedAreaImageWidthPixels");
    let full_width = exif.get_f64_ignoring_case("FullPanoWidthPixels");
    let has_gpano_dims = cropped_width.is_some() || full_width.is_some();

    let projection_type = exif
        .get_ignoring_case("ProjectionType")
        .and_then(|v| v.as_str())
        .map(str::to_lowercase);

    match projection_type.as_deref() {
        Some("equirectangular") => true,
        Some("cylindrical") => {
            // Only use the panorama viewer for cylindrical views if they cover 360 degrees
            if let (Some(cw), Some(fw)) = (cropped_width, full_width) {
                if fw > 0.0 {
                    let haov = (cw / fw) * 360.0;
                    haov >= 359.0
                } else {
                    false
                }
            } else {
                false
            }
        }
        None => {
            // When GPano tags are present but ProjectionType is omitted,
            // standard Google Photosphere specifications fallback to equirectangular.
            has_gpano_dims
        }
        _ => false,
    }
}
