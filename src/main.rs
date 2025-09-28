use exiftool::ExifTool;
use media_analyzer::data_url::file_to_data_url;
use media_analyzer::gps::get_gps_info;
use media_analyzer::time::get_time_info;
use media_analyzer::utils::list_files_walkdir_filtered;
use media_analyzer::weather::get_weather_info;
use meteostat::Meteostat;
use rand::prelude::IndexedRandom;
use rand::rng;
use std::path::Path;
use media_analyzer::analyze_result::AnalyzeResult;
use media_analyzer::tags::logic::extract_tags;
// TODO: make rust package
// add error handling
// add to output result:
// * pano info away from tags, to direct field
// * path
// * metadata(?):
//  width: int
//  height: int
//  duration: float | None
//  size_bytes: int
//  format: str
// camera_make
// camera_model
// iso
// exposure

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let mut et = ExifTool::new()?;
    let meteostat = Meteostat::new().await?;

    let start_dir = Path::new("E:/Backup/Photos/photos/photos");
    let all_files = list_files_walkdir_filtered(start_dir, false)?;
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
        let path = &file.canonicalize()?;
        // motion photo:
        // let path = Path::new("E:/Backup/Photos/Vakantie 2026 Sardinie/PXL_20250918_121421114.MP.jpg");

        // panorama
        // let path =
        //     Path::new("E:/Backup/Photos/Vakantie 2026 Sardinie/PXL_20250903_044134290.PANO.jpg");

        // video
        // let path = Path::new("E:/Backup/Photos/photos/photos/VID_20220723_134136.mp4");

        // burst
        // let path = Path::new("assets/timelapse.mp4");

        // weather info test
        // let path = Path::new("E:/Backup/Photos/photos/photos/PXL_20240917_131928676.jpg");

        opener::open(path).expect("panic message");

        let exif_info = et.json(path, &["-g2"])?;
        let numeric_exif = et.json(path, &["-n"])?;
        let tags = extract_tags(path, &numeric_exif);
        println!("{:?}", path);
        println!("{:?}", &tags);

        // println!("{}", serde_json::to_string_pretty(&numeric_exif).unwrap());

        let gps_info = get_gps_info(&numeric_exif).await;
        let time_info = get_time_info(&exif_info, gps_info.as_ref());
        let data_url = file_to_data_url(path)?;
        let weather_info = if let Some(ref gps) = gps_info {
            get_weather_info(&meteostat, gps, time_info.datetime_utc.unwrap())
                .await
                .ok()
                .flatten()
        } else {
            None
        };


        let analyze_result = AnalyzeResult{
            exif: exif_info,
            tags,
            time_info,
            weather_info,
            gps_info,
        };
        println!("{}", serde_json::to_string_pretty(&analyze_result)?);
    }

    Ok(())
}
