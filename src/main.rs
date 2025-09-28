use bon::bon;
use color_eyre::eyre::eyre;
use exiftool::ExifTool;
use media_analyzer::structs::AnalyzeResult;
use media_analyzer::tags::logic::extract_tags;
use media_analyzer::time::get_time_info;
use media_analyzer::utils::list_files_walkdir_filtered;
use meteostat::Meteostat;
use rand::prelude::IndexedRandom;
use rand::rng;
use std::path::{Path, PathBuf};
use std::time::Instant;
use media_analyzer::other::data_url::file_to_data_url;
use media_analyzer::other::gps::get_gps_info;
use media_analyzer::other::metadata::get_metadata;
use media_analyzer::other::pano::get_pano_info;
use media_analyzer::other::weather::get_weather_info;
// TODO: make rust package
// add error handling
// Reverse geolocation
// add gps accuracy to gpsInfo
// add bearing/movement speed to gpsInfo

pub struct MediaAnalyzer {
    exiftool: ExifTool,
    meteostat: Meteostat,
}

#[bon]
impl MediaAnalyzer {
    #[builder]
    pub async fn new(
        exiftool_path: Option<PathBuf>,
        cache_folder: Option<PathBuf>,
    ) -> color_eyre::Result<Self> {
        let exiftool = match exiftool_path {
            Some(exiftool_path) => ExifTool::with_executable(&exiftool_path)?,
            None => ExifTool::new()?,
        };
        let meteostat = match cache_folder {
            Some(cache_folder) => Meteostat::with_cache_folder(cache_folder).await?,
            None => Meteostat::new().await?,
        };
        Ok(Self {
            exiftool,
            meteostat,
        })
    }

    pub async fn analyze_media(
        &mut self,
        media_file: &Path,
        frames: Vec<&Path>,
    ) -> color_eyre::Result<AnalyzeResult> {
        let thumbnail_path = frames.first().ok_or(eyre!("No thumbnail frames"))?;
        let start_time = Instant::now();
        let data_url = file_to_data_url(thumbnail_path)?;
        let elapsed_time = start_time.elapsed();
        println!("[data_url] \t {:?}", elapsed_time);

        let start_time = Instant::now();
        let exif_info = self.exiftool.json(media_file, &["-g2"])?;
        let elapsed_time = start_time.elapsed();
        println!("[exif_info] \t {:?}", elapsed_time);

        let start_time = Instant::now();
        let numeric_exif = self.exiftool.json(media_file, &["-n"])?;
        let elapsed_time = start_time.elapsed();
        println!("[numeric_exif] \t {:?}", elapsed_time);

        let start_time = Instant::now();
        let metadata = get_metadata(&numeric_exif)?;
        let tags = extract_tags(media_file, &numeric_exif);
        let gps_info = get_gps_info(&numeric_exif).await;
        let time_info = get_time_info(&exif_info, gps_info.as_ref());
        let pano_info = get_pano_info(media_file, &numeric_exif);
        let elapsed_time = start_time.elapsed();
        println!("[many_data] \t {:?}", elapsed_time);

        let start_time = Instant::now();
        let weather_info = match gps_info {
            Some(ref gps) => {
                get_weather_info(&self.meteostat, gps, time_info.datetime_utc.unwrap())
                    .await
                    .ok()
            }
            None => None,
        };
        let elapsed_time = start_time.elapsed();
        println!("[weather_info] \t {:?}", elapsed_time);

        Ok(AnalyzeResult {
            exif: exif_info,
            tags,
            time_info,
            weather_info,
            gps_info,
            pano_info,
            data_url,
            metadata,
        })
    }
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let mut analyzer = MediaAnalyzer::builder().build().await?;

    let start_dir = Path::new("E:/Backup/Photos/photos/photos");
    let all_files = list_files_walkdir_filtered(start_dir, false)?;
    let sample_size = 1;
    let mut rng_machine = rng();
    let sampled_files_iter =
        all_files.choose_multiple(&mut rng_machine, sample_size.min(all_files.len()));

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

        // opener::open(path).expect("can't open photo");

        println!("\t{}", path.display());

        let analyze_result = analyzer.analyze_media(path, vec![path]).await?;

        println!("{}", serde_json::to_string_pretty(&analyze_result)?);
    }

    Ok(())
}
