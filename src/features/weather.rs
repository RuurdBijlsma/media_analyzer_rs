use crate::GpsInfo;
use crate::features::error::WeatherError;
use chrono::{DateTime, Utc};
use meteostat::RequiredData::SpecificDate;
use meteostat::{Hourly, LatLon, Meteostat};
use serde::{Deserialize, Serialize};
use sunrise::{Coordinates, DawnType, SolarDay, SolarEvent};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WeatherInfo {
    pub hourly: Option<Hourly>,
    pub sun_info: SunInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SunInfo {
    pub sunrise: DateTime<Utc>,
    pub sunset: DateTime<Utc>,
    pub dawn: DateTime<Utc>,
    pub dusk: DateTime<Utc>,
    pub is_daytime: bool,
}

// This internal function can now return a Result
fn compute_sun_info(datetime: DateTime<Utc>, gps_info: &GpsInfo) -> Result<SunInfo, WeatherError> {
    let date = datetime.date_naive();
    // The Coordinates::new can fail if lat/lon are invalid, though unlikely here.
    let coord = Coordinates::new(gps_info.latitude, gps_info.longitude)
        .ok_or(WeatherError::SunCalculationError)?;

    // These calls are infallible if Coordinates is valid.
    let sunrise = SolarDay::new(coord, date).event_time(SolarEvent::Sunrise);
    let sunset = SolarDay::new(coord, date).event_time(SolarEvent::Sunset);
    let dawn = SolarDay::new(coord, date).event_time(SolarEvent::Dawn(DawnType::Civil));
    let dusk = SolarDay::new(coord, date).event_time(SolarEvent::Dusk(DawnType::Civil));

    Ok(SunInfo {
        sunrise,
        sunset,
        dawn,
        dusk,
        is_daytime: datetime >= sunrise && datetime <= sunset,
    })
}

pub async fn get_weather_info(
    client: &Meteostat,
    gps_info: &GpsInfo,
    datetime: DateTime<Utc>,
    weather_search_radius_km: f64,
) -> Result<WeatherInfo, WeatherError> {
    // The '?' will convert meteostat::Error into our WeatherError::ApiError
    let hourly_call = client
        .hourly()
        .location(LatLon(gps_info.latitude, gps_info.longitude))
        .max_distance_km(weather_search_radius_km)
        .required_data(SpecificDate(datetime.date_naive()))
        .call()
        .await?;

    // Handle the case where there is data, but not for the specific hour requested
    let weather_info = hourly_call
        .get_at(datetime)
        .map_err(|_| WeatherError::NoDataAvailable)?
        .collect_single_hourly()
        .ok();

    // Use '?' on our fallible internal function
    let sun_info = compute_sun_info(datetime, gps_info)?;

    Ok(WeatherInfo {
        hourly: weather_info,
        sun_info,
    })
}
