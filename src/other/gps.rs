use crate::other::structs::{GpsInfo, LocationName};
use reverse_geocoder::ReverseGeocoder;
use serde_json::Value;

pub async fn get_gps_info(geocoder: &ReverseGeocoder, numeric_exif: &Value) -> Option<GpsInfo> {
    let (latitude, longitude) = match (
        numeric_exif.get("GPSLatitude").and_then(Value::as_f64),
        numeric_exif.get("GPSLongitude").and_then(Value::as_f64),
    ) {
        (Some(lat), Some(lon)) => (lat, lon),
        _ => return None,
    };
    let altitude = numeric_exif.get("GPSAltitude").and_then(Value::as_f64);

    let search_result = geocoder.search((latitude, longitude));
    let country_name = rust_iso3166::from_alpha2(&search_result.record.cc);
    let record = search_result.record;
    let location = LocationName {
        latitude: search_result.record.lat,
        longitude: search_result.record.lon,
        name: record.name.clone(),
        admin1: record.admin1.clone(),
        admin2: record.admin2.clone(),
        country_code: record.cc.clone(),
        country_name: country_name.map(|a| a.name.to_string()),
    };

    Some(GpsInfo {
        latitude,
        longitude,
        altitude,
        location,
    })
}
