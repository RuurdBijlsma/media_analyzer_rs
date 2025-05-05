use exiftool::ExifTool;
use media_file_analyzer::gps::get_gps_info;
use media_file_analyzer::time::get_time_info;
use media_file_analyzer::weather::get_weather_info;
use meteostat::Meteostat;
use std::error::Error;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let files = &["data/20180803_200256.jpg"];
    let mut et = ExifTool::new()?;
    let meteostat = Meteostat::new().await?;

    for file in files {
        let path = Path::new(file).canonicalize()?;

        let exif_info = et.json(&path, &["-g2"])?;
        let numeric_exif = et.json(&path, &["-n"])?;

        dbg!(&exif_info);

        let gps_info = get_gps_info(&numeric_exif).await;

        let time_info = get_time_info(&exif_info, gps_info.as_ref());

        if let Some(time_info) = time_info {
            dbg!(&time_info);
            if let Some(gps_info) = gps_info {
                let weather_info =
                    get_weather_info(&meteostat, gps_info, time_info.datetime_utc.unwrap()).await;
                dbg!(&weather_info);
            }
        }
    }

    Ok(())
}
