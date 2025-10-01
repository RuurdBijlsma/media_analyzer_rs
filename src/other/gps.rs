use reverse_geocoder::ReverseGeocoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub enum DirectionRef {
    TrueNorth,
    MagneticNorth,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct GpsInfo {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub location: LocationName,
    pub image_direction: Option<f64>,
    pub image_direction_ref: Option<DirectionRef>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct LocationName {
    pub latitude: f64,
    pub longitude: f64,
    pub name: String,
    pub admin1: String,
    pub admin2: String,
    pub country_code: String,
    pub country_name: Option<String>,
}

pub async fn get_gps_info(geocoder: &ReverseGeocoder, numeric_exif: &Value) -> Option<GpsInfo> {
    let (latitude, longitude) = match (
        numeric_exif.get("GPSLatitude").and_then(Value::as_f64),
        numeric_exif.get("GPSLongitude").and_then(Value::as_f64),
    ) {
        (Some(lat), Some(lon)) => (lat, lon),
        _ => return None,
    };
    let altitude = numeric_exif.get("GPSAltitude").and_then(Value::as_f64);
    let image_direction = numeric_exif.get("GPSImgDirection").and_then(Value::as_f64);
    let image_direction_ref = numeric_exif
        .get("GPSImgDirectionRef")
        .and_then(Value::as_str)
        .and_then(|s| match s {
            "T" => Some(DirectionRef::TrueNorth),
            "M" => Some(DirectionRef::MagneticNorth),
            _ => None,
        });

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
        image_direction,
        image_direction_ref,
    })
}
