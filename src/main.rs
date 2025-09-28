use exiftool::ExifTool;
use media_analyzer::analyze_result::AnalyzeResult;
use media_analyzer::data_url::file_to_data_url;
use media_analyzer::gps::get_gps_info;
use media_analyzer::pano::get_pano_info;
use media_analyzer::tags::logic::extract_tags;
use media_analyzer::time::get_time_info;
use media_analyzer::utils::list_files_walkdir_filtered;
use media_analyzer::weather::get_weather_info;
use meteostat::Meteostat;
use rand::prelude::IndexedRandom;
use rand::rng;
use std::path::{Path, PathBuf};
use bon::bon;
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
            None => ExifTool::new()?
        };
        let meteostat = match cache_folder {
            Some(cache_folder) => Meteostat::with_cache_folder(cache_folder).await?,
            None => Meteostat::new().await?
        };
        Ok(Self {
            exiftool,
            meteostat,
        })
    }


    pub async fn analyze_media(&mut self, media_file: &Path, frames: Vec<PathBuf>) ->color_eyre::Result<AnalyzeResult>{
        let exif_info = self.exiftool.json(media_file, &["-g2"])?;
        let numeric_exif = self.exiftool.json(media_file, &["-n"])?;
        let tags = extract_tags(media_file, &numeric_exif);
        println!("{:?}", media_file);
        println!("{:?}", &tags);

        // println!("{}", serde_json::to_string_pretty(&numeric_exif).unwrap());

        let gps_info = get_gps_info(&numeric_exif).await;
        let time_info = get_time_info(&exif_info, gps_info.as_ref());
        let pano_info = get_pano_info(media_file, &numeric_exif);
        let data_url = file_to_data_url(media_file)?;
        let weather_info = if let Some(ref gps) = gps_info {
            get_weather_info(&self.meteostat, gps, time_info.datetime_utc.unwrap())
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        let analyze_result = AnalyzeResult {
            exif: exif_info,
            tags,
            time_info,
            weather_info,
            gps_info,
            pano_info,
        };

        Ok(analyze_result)

    }
}


#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let mut analyzer = MediaAnalyzer::builder().build().await?;

    let start_dir = Path::new("E:/Backup/Photos/photos/photos");
    let all_files = list_files_walkdir_filtered(start_dir, false)?;
    println!("Found {} total files.", all_files.len());
    let sample_size = 1;
    let num_to_sample = sample_size.min(all_files.len());
    println!("Randomly sampling {} files...", num_to_sample);
    let mut rng_machine = rng();
    let sampled_files_iter = all_files.choose_multiple(&mut rng_machine, num_to_sample);

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
        let analyze_result  =analyzer.analyze_media(path, vec![]).await?;

        println!("{}", serde_json::to_string_pretty(&analyze_result)?);
    }

    Ok(())
}
