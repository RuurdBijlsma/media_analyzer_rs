use bon::bon;
use color_eyre::eyre::eyre;
use exiftool::ExifTool;
use meteostat::Meteostat;
use std::path::{Path, PathBuf};
use crate::other::data_url::file_to_data_url;
use crate::other::gps::get_gps_info;
use crate::other::metadata::get_metadata;
use crate::other::pano::get_pano_info;
use crate::other::weather::get_weather_info;
use crate::structs::AnalyzeResult;
use crate::tags::logic::extract_tags;
use crate::time::get_time_info;

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
        // Slow in general:
        let data_url = file_to_data_url(thumbnail_path)?;

        let exif_info = self.exiftool.json(media_file, &["-g2"])?;

        let numeric_exif = self.exiftool.json(media_file, &["-n"])?;

        let metadata = get_metadata(&numeric_exif)?;
        let tags = extract_tags(media_file, &numeric_exif);
        let gps_info = get_gps_info(&numeric_exif).await;
        let time_info = get_time_info(&exif_info, gps_info.as_ref());
        let pano_info = get_pano_info(media_file, &numeric_exif);

        // Slow if not cached:
        let weather_info = match gps_info {
            Some(ref gps) => {
                get_weather_info(&self.meteostat, gps, time_info.datetime_utc.unwrap())
                    .await
                    .ok()
            }
            None => None,
        };

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

