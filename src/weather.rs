use crate::gps::GpsInfo;
use chrono::{DateTime, Utc};
use color_eyre::eyre::bail;
use meteostat::RequiredData::SpecificDate;
use meteostat::{Hourly, LatLon, Meteostat, MeteostatError};

pub async fn get_weather_info(
    client: &Meteostat,
    gps_info: &GpsInfo,
    datetime: DateTime<Utc>,
) -> color_eyre::Result<Option<Hourly>> {
    let weather_info = client
        .hourly()
        .location(LatLon(gps_info.latitude, gps_info.longitude))
        .required_data(SpecificDate(datetime.date_naive()))
        .call()
        .await?
        .get_at(datetime)?
        .collect_single_hourly();
    match weather_info {
        Ok(weather_info) => Ok(Some(weather_info)),
        Err(MeteostatError::ExpectedSingleRow { actual: _ }) => Ok(None),
        Err(e) => bail!("Error getting weather: {}", e),
    }
}
