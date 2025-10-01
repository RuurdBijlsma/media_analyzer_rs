use crate::features::data_url::file_to_data_url;
use crate::features::gps::get_gps_info;
use crate::features::metadata::get_metadata;
use crate::features::pano::get_pano_info;
use crate::features::weather::get_weather_info;
use crate::structs::AnalyzeResult;
use crate::tags::logic::extract_tags;
use crate::time::get_time_info;
use crate::MediaAnalyzerError;
use bon::bon;
use exiftool::ExifTool;
use meteostat::Meteostat;
use reverse_geocoder::ReverseGeocoder;
use std::path::{Path, PathBuf};

pub struct MediaAnalyzer {
    geocoder: ReverseGeocoder,
    exiftool: ExifTool,
    meteostat: Meteostat,
}

#[bon]
impl MediaAnalyzer {
    /// Creates a new instance of `MediaAnalyzer`.
    ///
    /// This function initializes the `ExifTool` and `Meteostat` services.
    ///
    /// # Arguments
    ///
    /// * `exiftool_path` - An optional path to the `exiftool` executable. If `None`, the crate will attempt to find it in the system's PATH.
    /// * `cache_folder` - An optional path to a directory for caching `Meteostat` data. If `None`, a default cache location will be used.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// * `exiftool` cannot be found or initialized.
    /// * The `Meteostat` service cannot be initialized, for example, due to issues with the cache folder.
    #[builder]
    pub async fn new(
        exiftool_path: Option<PathBuf>,
        cache_folder: Option<PathBuf>,
    ) -> Result<Self, MediaAnalyzerError> {
        let exiftool = match exiftool_path {
            Some(path) => ExifTool::with_executable(&path)?,
            None => ExifTool::new()?,
        };
        let meteostat = match cache_folder {
            Some(path) => Meteostat::with_cache_folder(path).await?,
            None => Meteostat::new().await?,
        };
        let geocoder = ReverseGeocoder::new();
        Ok(Self {
            geocoder,
            exiftool,
            meteostat,
        })
    }

    /// Analyzes a media file to extract a wide range of information.
    ///
    /// This is the primary function of the crate. It takes a path to a media file
    /// and a list of frame paths (for generating a thumbnail) and returns an
    /// `AnalyzeResult` struct containing all the extracted data.
    ///
    /// # Arguments
    ///
    /// * `media_file` - A path to the video or photo file to be analyzed.
    /// * `frames` - A vector of paths to thumbnail frames. The first frame is used to generate a data URL.
    ///
    /// # Returns
    ///
    /// A `color_eyre::Result<AnalyzeResult>` which, on success, contains:
    /// * `exif` - Raw Exif data.
    /// * `tags` - Identified tags such as `is_motion_photo` or `is_night_sight`.
    /// * `time_info` - Time the media was taken, including timezone information.
    /// * `weather_info` - Weather data at the time and location of capture, including sunrise and sunset times.
    /// * `gps_info` - GPS location data, including the location's name.
    /// * `pano_info` - Information related to panoramic photos.
    /// * `data_url` - A base64-encoded data URL of the first thumbnail frame.
    /// * `metadata` - Basic information like width, height, duration, and MIME type.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// * No thumbnail frames are provided.
    /// * The thumbnail cannot be converted to a data URL.
    /// * `exiftool` fails to extract JSON data from the media file.
    pub async fn analyze_media(
        &mut self,
        media_file: &Path,
        frames: Vec<&Path>,
    ) -> Result<AnalyzeResult, MediaAnalyzerError> {
        // Return our crate's Error
        let thumbnail_path = frames.first().ok_or(MediaAnalyzerError::NoThumbnail)?;
        let data_url = file_to_data_url(thumbnail_path)?;

        let exif_info = self.exiftool.json(media_file, &["-g2"])?;
        let numeric_exif = self.exiftool.json(media_file, &["-n"])?;

        let (metadata, capture_details) = get_metadata(&numeric_exif)?;
        let tags = extract_tags(media_file, &numeric_exif);
        let gps_info = get_gps_info(&self.geocoder, &numeric_exif).await;
        let pano_info = get_pano_info(media_file, &numeric_exif);

        // This is now fallible, so we use '?'
        let time_info = get_time_info(&exif_info, gps_info.as_ref())?;

        // Get weather info only if we have both GPS and a valid UTC time.
        // We use .ok() to treat weather as "best-effort" and not fail the whole analysis.
        let weather_info =
            if let (Some(gps), Some(utc_time)) = (gps_info.as_ref(), time_info.datetime_utc) {
                get_weather_info(&self.meteostat, gps, utc_time).await.ok()
            } else {
                None
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
            capture_details,
        })
    }
}
