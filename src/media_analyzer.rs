use crate::MediaAnalyzerError;
use crate::features::gps::get_gps_info;
use crate::features::hashing::hash_file;
use crate::features::metadata::get_metadata;
use crate::features::pano::get_pano_info;
use crate::structs::AnalyzeResult;
use crate::tags::logic::extract_tags;
use crate::time::get_time_info;
use bon::bon;
use exiftool::ExifTool;
use reverse_geocoder::ReverseGeocoder;
use std::path::Path;

/// The main entry point for the media analysis pipeline.
///
/// This struct holds the initialized clients and configuration needed to perform
/// analysis. It is designed to be created once and reused for analyzing multiple files.
///
/// Use the builder pattern to construct an instance:
/// ```rust
/// # use media_analyzer::{MediaAnalyzer, MediaAnalyzerError};
/// # #[tokio::main]
/// # async fn main() -> Result<(), MediaAnalyzerError> {
/// let analyzer = MediaAnalyzer::builder()
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct MediaAnalyzer {
    geocoder: ReverseGeocoder,
    exiftool1: ExifTool,
    exiftool2: ExifTool,
}

#[bon]
impl MediaAnalyzer {
    /// Constructs a `MediaAnalyzer` via a builder pattern.
    ///
    /// This is the main constructor for the analyzer. It initializes all necessary services
    /// and allows for custom configuration of its behavior.
    ///
    /// # Builder Arguments
    ///
    /// * `exiftool_path: Option<PathBuf>` - An optional path to a specific `exiftool` executable. If `None`, `exiftool` will be searched for in the system's PATH.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// * The `exiftool` executable cannot be found or fails to start.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use media_analyzer::{MediaAnalyzer, MediaAnalyzerError};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), MediaAnalyzerError> {
    /// let analyzer = MediaAnalyzer::builder()
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[builder]
    pub fn new(exiftool_path: Option<&Path>) -> Result<Self, MediaAnalyzerError> {
        let exiftool1 = match exiftool_path {
            Some(path) => ExifTool::with_executable(path)?,
            None => ExifTool::new()?,
        };
        let exiftool2 = match exiftool_path {
            Some(path) => ExifTool::with_executable(path)?,
            None => ExifTool::new()?,
        };
        let geocoder = ReverseGeocoder::new();
        Ok(Self {
            geocoder,
            exiftool1,
            exiftool2,
        })
    }

    /// Analyzes a media file and extracts a set of metadata.
    ///
    /// This is the primary analysis function. It orchestrates all the individual parsing
    /// and data-gathering modules to produce a single, consolidated `AnalyzeResult`.
    ///
    /// # Arguments
    ///
    /// * `media_file` - A path to the video or photo file to be analyzed.
    /// * `thumbnail` - A path to an image file to be used for generating a thumbnail data URL. For photos, this can be the same path as `media_file`. For videos, this should be a path to an extracted frame.
    ///
    /// # Returns
    ///
    /// On success, returns a `Result` containing an [`AnalyzeResult`] struct with the following fields:
    /// * `exif`: The raw, grouped (`-g2`) JSON output from `exiftool`.
    /// * `metadata`: Core file properties like width, height, and duration.
    /// * `capture_details`: Photographic details like ISO, aperture, and camera model.
    /// * `tags`: Boolean flags for special media types (e.g., `is_motion_photo`, `is_slowmotion`).
    /// * `time_info`: Consolidated time information, including the best-guess UTC timestamp and timezone.
    /// * `pano_info`: Data related to panoramic images, including photospheres.
    /// * `gps_info`: GPS coordinates and reverse-geocoded location details.
    ///
    /// # Errors
    ///
    /// This function will return an error if any of the critical analysis steps fail, such as:
    /// * [`MediaAnalyzerError::DataUrl`]: The provided `thumbnail` path is invalid or not an image.
    /// * [`MediaAnalyzerError::Exiftool`]: `exiftool` fails to execute or read the `media_file`.
    /// * [`MediaAnalyzerError::Metadata`]: The `media_file` is missing essential metadata (e.g., `ImageWidth`).
    /// * [`MediaAnalyzerError::Time`]: No usable time information could be extracted from any source.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::path::Path;
    /// # use media_analyzer::{MediaAnalyzer, MediaAnalyzerError};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), MediaAnalyzerError> {
    /// let analyzer = MediaAnalyzer::builder().build()?;
    /// let photo_path = Path::new("assets/tent.jpg");
    ///
    /// // Analyze a photo, using the photo itself as the thumbnail source.
    /// let result = analyzer.analyze_media(photo_path).await?;
    ///
    /// println!("Photo taken in {:?}", result.gps_info.unwrap().location);
    /// println!("Camera Model: {}", result.capture_details.camera_model.unwrap_or_default());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn analyze_media(
        &self,
        media_file: &Path,
    ) -> Result<AnalyzeResult, MediaAnalyzerError> {
        let hash = hash_file(media_file)?;
        let exif_info = self.exiftool1.json(media_file, &["-g2"])?;
        let numeric_exif = self.exiftool2.json(media_file, &["-n"])?;

        let (metadata, capture_details) = get_metadata(&numeric_exif)?;
        let tags = extract_tags(media_file, &numeric_exif);
        let gps_info = get_gps_info(&self.geocoder, &numeric_exif).await;
        let pano_info = get_pano_info(media_file, &numeric_exif);

        let time_info = get_time_info(&exif_info, gps_info.as_ref())?;

        Ok(AnalyzeResult {
            hash,
            exif: exif_info,
            tags,
            time_info,
            gps_info,
            pano_info,
            metadata,
            capture_details,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MediaAnalyzerError;
    use std::path::{Path, PathBuf};

    /// A helper to get a specific asset path.
    fn asset_path(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join(relative)
    }

    #[tokio::test]
    async fn test_full_analysis_on_standard_jpg() -> Result<(), MediaAnalyzerError> {
        let analyzer = MediaAnalyzer::builder().build()?;
        let media_file = asset_path("sunset.jpg");

        // For a photo, the thumbnail is the file itself.
        let result = analyzer.analyze_media(&media_file).await?;

        // --- Assertions ---
        assert_eq!(result.metadata.width, 5312);
        assert!(!result.tags.is_video);
        assert!(!result.tags.is_hdr, "sunset.jpg is not hdr");
        assert!(result.gps_info.is_some(), "Should have GPS info");
        assert!(!result.tags.is_burst);
        assert!(!result.pano_info.is_photosphere);

        Ok(())
    }

    #[tokio::test]
    async fn test_on_hdr() -> Result<(), MediaAnalyzerError> {
        let analyzer = MediaAnalyzer::builder().build()?;
        let media_file = asset_path("hdr.jpg");

        // For a photo, the thumbnail is the file itself.
        let result = analyzer.analyze_media(&media_file).await?;

        // --- Assertions ---
        assert_eq!(result.metadata.width, 4032);
        assert!(!result.tags.is_video);
        assert!(result.tags.is_hdr, "hdr.jpg is hdr");
        assert!(result.gps_info.is_some(), "Should have GPS info");
        assert!(!result.tags.is_burst);
        assert!(!result.pano_info.is_photosphere);

        Ok(())
    }

    #[tokio::test]
    async fn test_on_heic() -> Result<(), MediaAnalyzerError> {
        let analyzer = MediaAnalyzer::builder().build()?;
        let media_file = asset_path("iphone.HEIC");

        // For a photo, the thumbnail is the file itself.
        let result = analyzer.analyze_media(&media_file).await?;

        // --- Assertions ---
        assert_eq!(result.metadata.width, 3024);
        assert_eq!(result.metadata.orientation, Some(6));
        assert!(!result.tags.is_video);
        assert!(!result.tags.is_hdr);
        assert!(result.gps_info.is_some(), "Should have GPS info");
        assert!(!result.tags.is_burst);
        assert!(!result.pano_info.is_photosphere);

        Ok(())
    }

    #[tokio::test]
    async fn test_full_analysis_on_standard_video() -> Result<(), MediaAnalyzerError> {
        let analyzer = MediaAnalyzer::builder().build()?;
        let media_file = asset_path("video/car.webm");

        let result = analyzer.analyze_media(&media_file).await?;

        // --- Assertions ---
        assert!(result.tags.is_video);
        assert!(result.metadata.duration.is_some());
        assert!(result.tags.video_fps.is_some());
        assert!(!result.tags.is_slowmotion);
        assert!(!result.tags.is_timelapse);
        assert!(!result.tags.is_motion_photo);
        assert!(!result.tags.is_hdr);

        Ok(())
    }

    #[tokio::test]
    async fn test_motion_photo_is_correctly_identified() -> Result<(), MediaAnalyzerError> {
        let analyzer = MediaAnalyzer::builder().build()?;
        let media_file = asset_path("motion/PXL_20250103_180944831.MP.jpg");

        let result = analyzer.analyze_media(&media_file).await?;

        // --- Assertions ---
        assert!(
            !result.tags.is_video,
            "Motion Photo is not a primary video file"
        );
        assert!(result.tags.is_motion_photo);
        assert!(result.tags.motion_photo_presentation_timestamp.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_photosphere_is_correctly_identified() -> Result<(), MediaAnalyzerError> {
        let analyzer = MediaAnalyzer::builder().build()?;
        let media_file = asset_path("photosphere.jpg");

        let result = analyzer.analyze_media(&media_file).await?;

        // --- Assertions ---
        assert!(result.pano_info.is_photosphere);
        assert!(result.pano_info.use_panorama_viewer);
        assert_eq!(
            result.pano_info.projection_type,
            Some("equirectangular".to_string())
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_night_sight_is_correctly_identified() -> Result<(), MediaAnalyzerError> {
        let analyzer = MediaAnalyzer::builder().build()?;
        let media_file = asset_path("night_sight/PXL_20250104_170020532.NIGHT.jpg");

        let result = analyzer.analyze_media(&media_file).await?;

        // --- Assertions ---
        assert!(result.tags.is_night_sight);

        Ok(())
    }

    #[tokio::test]
    async fn test_slow_motion_video_is_correctly_identified() -> Result<(), MediaAnalyzerError> {
        let analyzer = MediaAnalyzer::builder().build()?;
        let media_file = asset_path("slowmotion.mp4");
        // For video tests, we can just use any jpg as a placeholder thumbnail

        let result = analyzer.analyze_media(&media_file).await?;

        // --- Assertions ---
        assert!(result.tags.is_video);
        assert!(result.tags.is_slowmotion);
        assert!(!result.tags.is_timelapse);

        Ok(())
    }

    #[tokio::test]
    async fn test_timelapse_video_is_correctly_identified() -> Result<(), MediaAnalyzerError> {
        let analyzer = MediaAnalyzer::builder().build()?;
        let media_file = asset_path("timelapse.mp4");

        let result = analyzer.analyze_media(&media_file).await?;

        // --- Assertions ---
        assert!(result.tags.is_video);
        assert!(result.tags.is_timelapse);
        assert!(!result.tags.is_slowmotion);

        Ok(())
    }

    #[tokio::test]
    async fn test_analysis_fails_gracefully_for_non_media_file() -> Result<(), MediaAnalyzerError> {
        let analyzer = MediaAnalyzer::builder().build()?;
        let media_file = asset_path("text_file.txt");

        let result = analyzer.analyze_media(&media_file).await;

        // --- Assertions ---
        assert!(result.is_err(), "Analysis should fail for a non-media file");

        // Exiftool on a text file won't have required fields like ImageWidth,
        // so the `get_metadata` function should be the point of failure.
        assert!(
            matches!(result.unwrap_err(), MediaAnalyzerError::Metadata(_)),
            "The error should be a MetadataError"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_detailed_gps_time() -> Result<(), MediaAnalyzerError> {
        let analyzer = MediaAnalyzer::builder().build()?;
        let media_file = asset_path("sunset.jpg");

        let result = analyzer.analyze_media(&media_file).await?;

        // --- 1. GPS Info Assertions ---
        let gps_info = result
            .gps_info
            .as_ref()
            .expect("GPS info should be extracted for sunset.jpg");

        // Check coordinates (using approximate values)
        assert!((gps_info.latitude - 40.820_887_527_777_8).abs() < 0.001);
        assert!((gps_info.longitude - 14.422_816_666_666_7).abs() < 0.001);

        // Check reverse geocoded location data
        assert_eq!(gps_info.location.name, "Massa di Somma");
        assert_eq!(gps_info.location.admin1, "Campania");
        assert_eq!(gps_info.location.country_code, "IT");
        assert_eq!(gps_info.location.country_name, Some("Italy".to_string()));

        // --- 2. Time Info Assertions ---
        let time_info = result.time_info;

        // Check that the highest confidence method was used (Naive time + GPS location)
        assert_eq!(time_info.source_details.confidence, "High");
        assert_eq!(
            time_info.source_details.time_source,
            "SubSecDateTimeOriginal: Parsed SubSeconds"
        );

        // Check that UTC time and timezone were successfully calculated
        assert!(
            time_info.datetime_utc.is_some(),
            "UTC datetime should be calculated from naive and GPS"
        );
        assert!(
            time_info.timezone.is_some(),
            "Timezone should be determined from GPS"
        );

        let timezone = time_info.timezone.as_ref().unwrap();
        assert_eq!(timezone.name, "Europe/Rome");
        assert_eq!(
            timezone.offset_seconds, 7200,
            "Offset should be +2 hour for the photo's date"
        );

        Ok(())
    }
}
