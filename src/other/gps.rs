use crate::other::structs::GpsInfo;
use serde_json::Value;

pub async fn get_gps_info(numeric_exif: &Value) -> Option<GpsInfo> {
    let (latitude, longitude) = match (
        numeric_exif.get("GPSLatitude").and_then(Value::as_f64),
        numeric_exif.get("GPSLongitude").and_then(Value::as_f64),
    ) {
        (Some(lat), Some(lon)) => (lat, lon),
        _ => return None,
    };

    let altitude = numeric_exif.get("GPSAltitude").and_then(Value::as_f64);

    Some(GpsInfo {
        latitude,
        longitude,
        altitude,
    })
}
