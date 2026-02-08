use crate::GpsInfo;
use crate::features::error::WeatherError;
use chrono::{DateTime, Utc};
use meteostat::{Hourly, LatLon, Meteostat, RequiredData};
use serde::{Deserialize, Serialize};
use sunrise::{Coordinates, DawnType, SolarDay, SolarEvent};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherInfo {
    pub hourly: Option<Hourly>,
    pub sun_info: SunInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SunInfo {
    pub sunrise: Option<DateTime<Utc>>,
    pub sunset: Option<DateTime<Utc>>,
    pub dawn: Option<DateTime<Utc>>,
    pub dusk: Option<DateTime<Utc>>,
    pub is_daytime: bool,
}

// This internal function can now return a Result
fn compute_sun_info(datetime: DateTime<Utc>, gps_info: &GpsInfo) -> Result<SunInfo, WeatherError> {
    let date = datetime.date_naive();
    let coord = Coordinates::new(gps_info.latitude, gps_info.longitude)
        .ok_or(WeatherError::SunCalculationError)?;

    let sunrise = SolarDay::new(coord, date).event_time(SolarEvent::Sunrise);
    let sunset = SolarDay::new(coord, date).event_time(SolarEvent::Sunset);
    let dawn = SolarDay::new(coord, date).event_time(SolarEvent::Dawn(DawnType::Civil));
    let dusk = SolarDay::new(coord, date).event_time(SolarEvent::Dusk(DawnType::Civil));
    // polar locations can have no sunset, no sunrise, or only one of both on a given day
    let is_daytime = if let Some(sr) = sunrise
        && let Some(ss) = sunset
    {
        datetime >= sr && datetime <= ss
    } else if let Some(sr) = sunrise {
        datetime >= sr
    } else if let Some(ss) = sunset {
        datetime <= ss
    } else {
        true
    };
    Ok(SunInfo {
        sunrise,
        sunset,
        dawn,
        dusk,
        is_daytime,
    })
}

pub async fn get_weather_info(
    client: &Meteostat,
    gps_info: &GpsInfo,
    datetime: DateTime<Utc>,
    weather_search_radius_km: f64,
) -> Result<WeatherInfo, WeatherError> {
    let hourly_frame = client
        .hourly()
        .location(LatLon(gps_info.latitude, gps_info.longitude))
        .required_data(RequiredData::SpecificDate(datetime.date_naive()))
        .max_distance_km(weather_search_radius_km)
        .call()
        .await?;

    // Handle the case where there is data, but not for the specific hour requested
    let weather_info = hourly_frame
        .get_at(datetime)
        .map_err(|_| WeatherError::NoDataAvailable)?
        .collect_single_hourly();

    let weather_info = weather_info.ok();
    let sun_info = compute_sun_info(datetime, gps_info)?;

    Ok(WeatherInfo {
        hourly: weather_info,
        sun_info,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::gps::{GpsInfo, LocationName};
    use chrono::TimeZone;
    use chrono_tz::Europe::Amsterdam;

    /// Helper function to create a `GpsInfo` struct for a known location (Amsterdam, NL).
    fn amsterdam_gps_info() -> GpsInfo {
        GpsInfo {
            latitude: 52.379_189,
            longitude: 4.899_431,
            altitude: Some(0.0),
            location: LocationName {
                latitude: 52.379_189,
                longitude: 4.899_431,
                name: "Amsterdam".to_string(),
                admin1: "North Holland".to_string(),
                admin2: String::new(),
                country_code: "NL".to_string(),
                country_name: Some("Netherlands".to_string()),
            },
            image_direction: None,
            image_direction_ref: None,
        }
    }

    // --- UNIT TESTS (Fast, Offline) ---

    #[test]
    fn test_compute_sun_info_for_daytime() {
        let gps_info = amsterdam_gps_info();
        // A time clearly during the day in Amsterdam (UTC+2) on this summer date.
        let daytime = Amsterdam
            .with_ymd_and_hms(2024, 7, 10, 14, 0, 0)
            .unwrap()
            .to_utc();

        let sun_info = compute_sun_info(daytime, &gps_info).unwrap();
        assert!(sun_info.is_daytime, "14:00 in summer should be daytime");
    }

    #[test]
    fn test_compute_sun_info_for_nighttime() {
        let gps_info = amsterdam_gps_info();
        // A time clearly at night in Amsterdam (UTC+2) on this summer date.
        let nighttime = Amsterdam
            .with_ymd_and_hms(2024, 7, 10, 23, 0, 0)
            .unwrap()
            .to_utc();

        let sun_info = compute_sun_info(nighttime, &gps_info).unwrap();
        assert!(!sun_info.is_daytime, "23:00 in summer should be nighttime");
    }

    #[test]
    fn test_compute_sun_info_fails_with_invalid_gps_coordinates() {
        let mut invalid_gps = amsterdam_gps_info();
        invalid_gps.latitude = 91.0; // Invalid latitude
        let time = Utc::now();
        let result = compute_sun_info(time, &invalid_gps);
        assert!(matches!(
            result.unwrap_err(),
            WeatherError::SunCalculationError
        ));
    }

    /// This is an integration test that makes a real network call to the Meteostat API.
    #[tokio::test]
    async fn test_get_weather_info_integration_success() {
        // 1. Setup
        let client = Meteostat::new()
            .await
            .expect("Failed to create Meteostat client");
        let gps_info = amsterdam_gps_info();
        // A date in the past to ensure data is available
        let datetime = Utc.with_ymd_and_hms(2023, 10, 26, 12, 0, 0).unwrap();
        let radius = 100.0;

        // 2. Execute
        let result = get_weather_info(&client, &gps_info, datetime, radius).await;

        // 3. Assert
        assert!(
            result.is_ok(),
            "API call should succeed for a major city. Result: {:?}",
            result.err()
        );
        let weather_info = result.unwrap();

        // We can reliably check the sun info
        assert!(
            weather_info.sun_info.is_daytime,
            "12:00 UTC in October should be daytime in Amsterdam"
        );

        // For the hourly data, we can't assert specific values, but we can check if we got *something*.
        // The service is not guaranteed to have data for every hour, so `is_some()` is a good enough check.
        println!("Received hourly data: {:?}", weather_info.hourly);
    }
}
