use serde_json::Value;

pub fn get_fps(value: &Value) -> (Option<f64>, Option<f64>) {
    let video_fps = value
        .get("AvgFrameRate")
        .or_else(|| value.get("FrameRate"))
        .or_else(|| value.get("VideoFrameRate"))
        .and_then(parse_fps);

    let capture_fps = value
        .get("AndroidCaptureFPS")
        .or_else(|| value.get("SourceFrameRate"))
        .and_then(parse_fps)
        .or(video_fps);

    (video_fps, capture_fps)
}

/// Parses frame rate values which can be integers, floats, or fractions (e.g., "30000/1001").
pub fn parse_fps(value: &Value) -> Option<f64> {
    if let Some(fps_str) = value.as_str() {
        if fps_str.contains('/') {
            let parts: Vec<&str> = fps_str.split('/').collect();
            if parts.len() == 2
                && let (Ok(num), Ok(den)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>())
                && den != 0.0
            {
                return Some(num / den);
            }
        } else if let Ok(fps) = fps_str.parse::<f64>() {
            return Some(fps);
        }
    } else if let Some(fps) = value.as_f64() {
        return Some(fps);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- Unit tests for the helper `parse_fps` function ---
    mod parse_fps_tests {
        use super::*;

        #[test]
        fn test_parses_json_number_float() {
            let value = json!(29.97);
            assert!((parse_fps(&value).unwrap() - 29.97).abs() < 1e-9);
        }

        #[test]
        fn test_parses_json_number_integer() {
            let value = json!(60);
            assert!((parse_fps(&value).unwrap() - 60.0).abs() < 1e-9);
        }

        #[test]
        fn test_parses_json_string_float() {
            let value = json!("24.0");
            assert!((parse_fps(&value).unwrap() - 24.0).abs() < 1e-9);
        }

        #[test]
        fn test_parses_json_string_fraction() {
            // Standard NTSC broadcast frame rate
            let value = json!("30000/1001");
            assert!((parse_fps(&value).unwrap() - 29.970_029_97).abs() < 1e-9);
        }

        #[test]
        fn test_returns_none_for_malformed_string() {
            let value = json!("not a number");
            assert!(parse_fps(&value).is_none());
        }

        #[test]
        fn test_returns_none_for_division_by_zero_fraction() {
            let value = json!("30000/0");
            assert!(parse_fps(&value).is_none());
        }

        #[test]
        fn test_returns_none_for_malformed_fraction() {
            let value = json!("30000/1001/2");
            assert!(parse_fps(&value).is_none());
        }

        #[test]
        fn test_returns_none_for_unsupported_json_type() {
            assert!(parse_fps(&json!(null)).is_none());
            assert!(parse_fps(&json!(true)).is_none());
            assert!(parse_fps(&json!([])).is_none());
            assert!(parse_fps(&json!({})).is_none());
        }
    }

    // --- Unit tests for the main `get_fps` function ---
    mod get_fps_tests {
        use super::*;

        #[test]
        fn test_gets_primary_video_and_capture_fps() {
            let exif = json!({
                "AvgFrameRate": 30.0,
                "AndroidCaptureFPS": "120"
            });
            let (video_fps, capture_fps) = get_fps(&exif);
            assert_eq!(video_fps, Some(30.0));
            assert_eq!(capture_fps, Some(120.0));
        }

        #[test]
        fn test_video_fps_fallback_logic() {
            // AvgFrameRate is missing, should fall back to FrameRate
            let exif_fallback1 = json!({ "FrameRate": 25.0 });
            let (video_fps1, _) = get_fps(&exif_fallback1);
            assert_eq!(video_fps1, Some(25.0));

            // FrameRate is missing, should fall back to VideoFrameRate
            let exif_fallback2 = json!({ "VideoFrameRate": "50" });
            let (video_fps2, _) = get_fps(&exif_fallback2);
            assert_eq!(video_fps2, Some(50.0));
        }

        #[test]
        fn test_capture_fps_fallback_to_sourceframerate() {
            let exif = json!({
                "AvgFrameRate": 30.0,
                "SourceFrameRate": 240.0
            });
            let (video_fps, capture_fps) = get_fps(&exif);
            assert_eq!(video_fps, Some(30.0));
            assert_eq!(capture_fps, Some(240.0));
        }

        #[test]
        fn test_capture_fps_falls_back_to_video_fps() {
            // This is the most important logic: if no specific capture FPS tags are found,
            // capture_fps should equal video_fps.
            let exif = json!({
                "AvgFrameRate": "30000/1001"
            });
            let (video_fps, capture_fps) = get_fps(&exif);
            assert!(video_fps.is_some());
            // They should be exactly equal
            assert_eq!(video_fps, capture_fps);
        }

        #[test]
        fn test_returns_none_when_no_fps_tags_present() {
            let exif = json!({ "ImageWidth": 1024 }); // Some other tag
            let (video_fps, capture_fps) = get_fps(&exif);
            assert!(video_fps.is_none());
            assert!(capture_fps.is_none());
        }

        #[test]
        fn test_handles_mixed_data_types() {
            let exif = json!({
                "AvgFrameRate": "60",         // String integer
                "AndroidCaptureFPS": 240.0,  // JSON number
                "SourceFrameRate": "120/1"   // String fraction
            });
            // It should prefer AndroidCaptureFPS over SourceFrameRate
            let (video_fps, capture_fps) = get_fps(&exif);
            assert_eq!(video_fps, Some(60.0));
            assert_eq!(capture_fps, Some(240.0));
        }
    }
}
