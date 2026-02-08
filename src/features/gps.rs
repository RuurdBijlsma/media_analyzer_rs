use reverse_geocoder::ReverseGeocoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum DirectionRef {
    TrueNorth,
    MagneticNorth,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GpsInfo {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
    pub location: LocationName,
    pub image_direction: Option<f64>,
    pub image_direction_ref: Option<DirectionRef>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationName {
    pub latitude: f64,
    pub longitude: f64,
    pub name: String,
    pub admin1: String,
    pub admin2: String,
    pub country_code: String,
    pub country_name: Option<String>,
}

pub fn get_gps_info(geocoder: &ReverseGeocoder, numeric_exif: &Value) -> Option<GpsInfo> {
    let (Some(latitude), Some(longitude)) = (
        numeric_exif.get("GPSLatitude").and_then(Value::as_f64),
        numeric_exif.get("GPSLongitude").and_then(Value::as_f64),
    ) else {
        return None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use reverse_geocoder::ReverseGeocoder;
    use serde_json::json;

    #[tokio::test]
    async fn test_get_gps_info_with_full_data() {
        let geocoder = ReverseGeocoder::new();
        // Simulate numeric exif data with all relevant GPS tags
        let numeric_exif = json!({
            "GPSLatitude": 52.379_189,
            "GPSLongitude": 4.899_431,
            "GPSAltitude": 10.5,
            "GPSImgDirection": 123.45,
            "GPSImgDirectionRef": "T"
        });

        let result = get_gps_info(&geocoder, &numeric_exif);

        // 1. Assert that we got a result
        assert!(result.is_some(), "Should return Some for valid GPS data");
        let gps_info = result.unwrap();

        // 2. Assert the direct values were parsed correctly
        assert_eq!(gps_info.latitude, 52.379_189);
        assert_eq!(gps_info.longitude, 4.899_431);
        assert_eq!(gps_info.altitude, Some(10.5));
        assert_eq!(gps_info.image_direction, Some(123.45));
        assert_eq!(gps_info.image_direction_ref, Some(DirectionRef::TrueNorth));

        // 3. Assert that the reverse geocoding worked as expected
        let location = gps_info.location;
        assert_eq!(location.name, "Amsterdam");
        assert_eq!(location.admin1, "North Holland");
        assert_eq!(location.country_code, "NL");
        assert_eq!(location.country_name, Some("Netherlands".to_string()));
    }

    #[tokio::test]
    async fn test_get_gps_info_with_minimal_data() {
        let geocoder = ReverseGeocoder::new();
        // Simulate numeric exif data with only the required tags
        let numeric_exif = json!({
            "GPSLatitude": 40.7128,
            "GPSLongitude": -74.0060
        });

        let result = get_gps_info(&geocoder, &numeric_exif);

        // 1. Assert that we still get a result
        assert!(result.is_some(), "Should return Some for minimal GPS data");
        let gps_info = result.unwrap();

        // 2. Assert the core values are correct
        assert_eq!(gps_info.latitude, 40.7128);
        assert_eq!(gps_info.longitude, -74.0060);

        // 3. Assert the optional values are correctly set to None
        assert!(gps_info.altitude.is_none());
        assert!(gps_info.image_direction.is_none());
        assert!(gps_info.image_direction_ref.is_none());

        // 4. Assert geocoding still worked
        assert_eq!(gps_info.location.name, "New York City");
        assert_eq!(gps_info.location.country_code, "US");
    }

    #[tokio::test]
    async fn test_returns_none_if_latitude_is_missing() {
        let geocoder = ReverseGeocoder::new();
        // Longitude is present, but latitude is missing
        let numeric_exif = json!({
            "GPSLongitude": 4.899_431,
        });

        let result = get_gps_info(&geocoder, &numeric_exif);
        assert!(
            result.is_none(),
            "Should return None when GPSLatitude is missing"
        );
    }

    #[tokio::test]
    async fn test_returns_none_if_longitude_is_missing() {
        let geocoder = ReverseGeocoder::new();
        // Latitude is present, but longitude is missing
        let numeric_exif = json!({
            "GPSLatitude": 52.379_189,
        });

        let result = get_gps_info(&geocoder, &numeric_exif);
        assert!(
            result.is_none(),
            "Should return None when GPSLongitude is missing"
        );
    }

    #[tokio::test]
    async fn test_returns_none_for_empty_exif_data() {
        let geocoder = ReverseGeocoder::new();
        let numeric_exif = json!({}); // Empty JSON object

        let result = get_gps_info(&geocoder, &numeric_exif);
        assert!(result.is_none(), "Should return None for empty EXIF data");
    }
}
