use crate::ExifData;
use reverse_geocoder::ReverseGeocoder;
use serde::{Deserialize, Serialize};

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

pub fn get_gps_info(geocoder: &ReverseGeocoder, exif: &ExifData) -> Option<GpsInfo> {
    let (Some(latitude), Some(longitude)) =
        (exif.get_f64("GPSLatitude"), exif.get_f64("GPSLongitude"))
    else {
        return None;
    };
    if latitude == 0.0 && longitude == 0.0 {
        return None;
    }
    let altitude = extract_altitude(exif);
    let image_direction = exif.get_f64("GPSImgDirection");
    let image_direction_ref = exif.get_str("GPSImgDirectionRef").and_then(|s| match s {
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

fn extract_altitude(exif: &ExifData) -> Option<f64> {
    let raw_alt = exif.group_f64("Location", "GPSAltitude");

    if let Some(raw_alt) = raw_alt {
        let unsigned_alt = raw_alt.abs();
        let ref_num = exif.group_f64("Location", "GPSAltitudeRef");
        let ref_str = exif.group_str("Location", "GPSAltitudeRef");

        // Strictly validate GPSAltitudeRef
        let is_below_sea_level = ref_num.map_or_else(
            || {
                ref_str.is_some_and(|ref_str| {
                    let ref_str_lower = ref_str.to_lowercase();
                    ref_str_lower.contains("below") || ref_str_lower.contains("negative")
                })
            },
            |ref_num| (ref_num - 1.0).abs() < 1e-5 || (ref_num - 3.0).abs() < 1e-5,
        );

        let alt = if is_below_sea_level {
            -unsigned_alt
        } else {
            unsigned_alt
        };
        return Some(alt);
    }

    // Fall back to standard composite lookup
    exif.get_f64("GPSAltitude")
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
        let exif = ExifData::new(json!({
            "GPSLatitude": 52.379_189,
            "GPSLongitude": 4.899_431,
            "GPSAltitude": 10.5,
            "GPSImgDirection": 123.45,
            "GPSImgDirectionRef": "T"
        }));

        let result = get_gps_info(&geocoder, &exif);

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
        let exif = ExifData::new(json!({
            "GPSLatitude": 40.7128,
            "GPSLongitude": -74.0060
        }));

        let result = get_gps_info(&geocoder, &exif);

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
        let exif = ExifData::new(json!({
            "GPSLongitude": 4.899_431,
        }));

        let result = get_gps_info(&geocoder, &exif);
        assert!(
            result.is_none(),
            "Should return None when GPSLatitude is missing"
        );
    }

    #[tokio::test]
    async fn test_returns_none_if_longitude_is_missing() {
        let geocoder = ReverseGeocoder::new();
        // Latitude is present, but longitude is missing
        let exif = ExifData::new(json!({
            "GPSLatitude": 52.379_189,
        }));

        let result = get_gps_info(&geocoder, &exif);
        assert!(
            result.is_none(),
            "Should return None when GPSLongitude is missing"
        );
    }

    #[tokio::test]
    async fn test_returns_none_for_empty_exif_data() {
        let geocoder = ReverseGeocoder::new();
        let exif = ExifData::new(json!({})); // Empty JSON object

        let result = get_gps_info(&geocoder, &exif);
        assert!(result.is_none(), "Should return None for empty EXIF data");
    }

    #[tokio::test]
    async fn test_gps_altitude_lg_g4_bug() {
        let geocoder = ReverseGeocoder::new();
        // Simulate the grouped structure returned by exiftool -n -g2
        let exif = ExifData::new(json!({
            "Location": {
                "GPSLatitude": 42.540,
                "GPSLongitude": 1.7138,
                "GPSAltitude": 2401,
                "GPSAltitudeRef": 1.8
            },
            "Composite": {
                "GPSLatitude": 42.540,
                "GPSLongitude": 1.7138,
                "GPSAltitude": -2401 // ExifTool incorrectly negated this
            }
        }));

        let result = get_gps_info(&geocoder, &exif);
        assert!(result.is_some());
        let gps_info = result.unwrap();
        // Altitude should remain positive (invalid 1.8 is ignored)
        assert_eq!(gps_info.altitude, Some(2401.0));
    }

    #[tokio::test]
    async fn test_gps_altitude_below_sea_level() {
        let geocoder = ReverseGeocoder::new();
        // Simulate the grouped structure for actual below-sea-level values
        let exif = ExifData::new(json!({
            "Location": {
                "GPSLatitude": 38.629,
                "GPSLongitude": 20.610,
                "GPSAltitude": 4,
                "GPSAltitudeRef": 1
            },
            "Composite": {
                "GPSLatitude": 38.629,
                "GPSLongitude": 20.610,
                "GPSAltitude": -4
            }
        }));

        let result = get_gps_info(&geocoder, &exif);
        assert!(result.is_some());
        let gps_info = result.unwrap();
        // Altitude should correctly remain negative
        assert_eq!(gps_info.altitude, Some(-4.0));
    }
}
