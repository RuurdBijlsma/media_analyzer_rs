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
