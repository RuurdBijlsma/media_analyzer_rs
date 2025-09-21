use exiftool::ExifTool;
use media_analyzer::gps::get_gps_info;
use media_analyzer::time::get_time_info;
use media_analyzer::utils::list_files_walkdir_filtered;
use media_analyzer::weather::get_weather_info;
use meteostat::Meteostat;
use rand::prelude::IndexedRandom;
use rand::rng;
use std::path::Path;
use media_analyzer::data_url::file_to_data_url;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let mut et = ExifTool::new()?;
    let meteostat = Meteostat::new().await?;

    let start_dir = Path::new("E:/Backup/Photos/photos/photos");
    let all_files = list_files_walkdir_filtered(start_dir, false)?; // Renamed to avoid confusion
    println!("Found {} total files.", all_files.len());
    let sample_size = 1;
    let num_to_sample = sample_size.min(all_files.len());
    println!("Randomly sampling {} files...", num_to_sample);
    let mut rng_machine = rng();
    // choose_multiple returns an iterator over references (&PathBuf)
    let sampled_files_iter = all_files.choose_multiple(&mut rng_machine, num_to_sample);
    // --- End Sampling Logic ---

    println!("Processing sampled files:");

    // Iterate over the sampled files
    for file in sampled_files_iter {
        let path = file.canonicalize()?;

        let exif_info = et.json(&path, &["-g2"])?;
        let numeric_exif = et.json(&path, &["-n"])?;
        let gps_info = get_gps_info(&numeric_exif).await;
        let time_info = get_time_info(&exif_info, gps_info.as_ref());
        let data_url = file_to_data_url(&path);
        println!("{:?}", data_url);

        if let Some(time_info) = &time_info && let Some(gps_info) = &gps_info {
            let weather_info =
                get_weather_info(&meteostat, gps_info, time_info.datetime_utc.unwrap())
                    .await
                    .ok()
                    .flatten();
            println!(
                "{} - UTC: {:?}, NAIVE: {}, TEMP: {:?}, GPS: {:?}",
                path.display(),
                time_info.datetime_utc,
                time_info.datetime_naive,
                weather_info.and_then(|x| x.temperature),
                gps_info,
            );
        } else if let Some(time_info) = &time_info {
            println!(
                "{} - UTC: {:?}, NAIVE: {}",
                path.display(),
                time_info.datetime_utc,
                time_info.datetime_naive,
            );
        } else {
            println!("{} - NO TIMEINFO", path.display());
        }
    }

    Ok(())
}
