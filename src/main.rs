use chrono::{DateTime, NaiveDate, Utc};
use exiftool::ExifTool;
use meteostat::{LatLon, Meteostat};
use std::error::Error;
use std::path::Path;
use meteostat::RequiredData::SpecificDate;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let files = &["data/20180803_200256.jpg"];
    let mut et = ExifTool::new()?;
    let meteostat = Meteostat::new().await?;

    for file in files {
        let path = Path::new(file).canonicalize()?;

        let exif_info = et.json(&path, &["-g2"])?;
        let numeric_exif = et.json(&path, &["-n"])?;

        let latitude = numeric_exif.get("GPSLatitude");
        let longitude = numeric_exif.get("GPSLongitude");
        let altitude = numeric_exif.get("GPSAltitude");

        let img_time = numeric_exif.get("GPSDateTime");

        if let (Some(latitude), Some(longitude), Some(datetime)) = (
            latitude.and_then(|x| x.as_f64()),
            longitude.and_then(|x| x.as_f64()),
            img_time.and_then(|x| x.as_str()),
        ) {
            let datetime_fixed = datetime.replace('Z', "+00:00");
            let dt = DateTime::parse_from_str(&datetime_fixed, "%Y:%m:%d %H:%M:%S%:z")?;
            let dt_utc: DateTime<Utc> = dt.with_timezone(&Utc);
            dbg!(&latitude, &longitude, &dt_utc);
            let weather_info = meteostat
                .hourly()
                .location(LatLon(latitude, longitude))
                .required_data(SpecificDate(dt.date_naive()))
                .call()
                .await?
                .get_at(dt_utc)?
                .collect_hourly();
            dbg!(&weather_info);
        }
    }

    Ok(())
}
