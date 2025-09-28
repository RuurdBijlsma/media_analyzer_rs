use crate::other::structs::{GpsInfo, SunInfo, WeatherInfo};
use chrono::{DateTime, Utc};
use meteostat::RequiredData::SpecificDate;
use meteostat::{LatLon, Meteostat};
use sunrise::{Coordinates, DawnType, SolarDay, SolarEvent};

fn compute_sun_info(datetime: DateTime<Utc>, gps_info: &GpsInfo) -> Option<SunInfo> {
    let date = datetime.date_naive();
    let coord = Coordinates::new(gps_info.latitude, gps_info.longitude)?;

    let sunrise = SolarDay::new(coord, date)
        .with_altitude(gps_info.altitude.unwrap_or(0.0))
        .event_time(SolarEvent::Sunrise);

    let sunset = SolarDay::new(coord, date)
        .with_altitude(gps_info.altitude.unwrap_or(0.0))
        .event_time(SolarEvent::Sunset);

    let dawn = SolarDay::new(coord, date)
        .with_altitude(gps_info.altitude.unwrap_or(0.0))
        .event_time(SolarEvent::Dawn(DawnType::Civil));

    let dusk = SolarDay::new(coord, date)
        .with_altitude(gps_info.altitude.unwrap_or(0.0))
        .event_time(SolarEvent::Dusk(DawnType::Civil));

    Some(SunInfo {
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
) -> color_eyre::Result<WeatherInfo> {
    let weather_info = client
        .hourly()
        .location(LatLon(gps_info.latitude, gps_info.longitude))
        .max_distance_km(100.)
        .required_data(SpecificDate(datetime.date_naive()))
        .call()
        .await?
        .get_at(datetime)?
        .collect_single_hourly();
    Ok(WeatherInfo{
        hourly: weather_info.ok(),
        sun_info: compute_sun_info(datetime, gps_info).expect("I don't think this will fail.")
    })
}
