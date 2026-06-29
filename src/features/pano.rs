use serde_json::Value;

/// Helper to search for an EXIF tag case-insensitively, handling optional namespace prefixes.
fn get_tag_value<'a>(exif: &'a Value, tag_name: &str) -> Option<&'a Value> {
    let obj = exif.as_object()?;
    if let Some(val) = obj.get(tag_name) {
        return Some(val);
    }

    // Case-insensitive fallback and namespace suffix matching
    let target = tag_name.to_lowercase();
    for (key, val) in obj {
        let key_lower = key.to_lowercase();
        if key_lower == target || key_lower.ends_with(&format!(":{target}")) {
            return Some(val);
        }
    }
    None
}

/// Safely converts an EXIF value to a float.
fn get_f64_tag(exif: &Value, tag_name: &str) -> Option<f64> {
    let val = get_tag_value(exif, tag_name)?;
    if let Some(n) = val.as_f64() {
        return Some(n);
    }
    if let Some(s) = val.as_str() {
        return s.parse::<f64>().ok();
    }
    None
}

/// Safely converts an EXIF value to a boolean.
fn as_bool(val: &Value) -> Option<bool> {
    if let Some(b) = val.as_bool() {
        return Some(b);
    }
    if let Some(s) = val.as_str() {
        let s_lower = s.to_lowercase();
        if s_lower == "true" || s_lower == "1" || s_lower == "yes" {
            return Some(true);
        }
        if s_lower == "false" || s_lower == "0" || s_lower == "no" {
            return Some(false);
        }
    }
    if let Some(n) = val.as_f64() {
        return Some(n == 1.0);
    }
    if let Some(n) = val.as_i64() {
        return Some(n == 1);
    }
    None
}

pub fn should_use_pano_viewer(exif: &Value) -> bool {
    if let Some(use_panorama_viewer) = get_tag_value(exif, "UsePanoramaViewer").and_then(as_bool) {
        return use_panorama_viewer;
    }

    // 3. Extract dimensions and projection metadata
    let cropped_width = get_f64_tag(exif, "CroppedAreaImageWidthPixels");
    let full_width = get_f64_tag(exif, "FullPanoWidthPixels");
    let has_gpano_dims = cropped_width.is_some() || full_width.is_some();

    let projection_type = get_tag_value(exif, "ProjectionType")
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
