use exiftool::ExifTool;
use media_analyzer::data_url::file_to_data_url;
use media_analyzer::gps::get_gps_info;
use media_analyzer::tags::extract_tags;
use media_analyzer::time::get_time_info;
use media_analyzer::utils::list_files_walkdir_filtered;
use media_analyzer::weather::get_weather_info;
use meteostat::Meteostat;
use rand::prelude::IndexedRandom;
use rand::rng;
use std::path::Path;

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
        // motion photo:
        // let path = Path::new("E:/Backup/Photos/Vakantie 2026 Sardinie/PXL_20250918_121421114.MP.jpg");

        // panorama
        // let path = Path::new("E:/Backup/Photos/Vakantie 2026 Sardinie/PXL_20250903_044134290.PANO.jpg");

        // video
        // let path = Path::new("E:/Backup/Photos/photos/photos/VID_20220723_134136.mp4");

        // burst
        let path = Path::new("assets/timelapse.mp4");

        opener::open(path).expect("panic message");

        let exif_info = et.json(path, &["-g2"])?;
        let numeric_exif = et.json(path, &["-n"])?;
        let tags = extract_tags(path, &numeric_exif);
        println!("{:?}", &tags);

        // println!("{}", serde_json::to_string_pretty(&numeric_exif).unwrap());

        let gps_info = get_gps_info(&numeric_exif).await;
        let time_info = get_time_info(&exif_info, gps_info.as_ref());
        let _data_url = file_to_data_url(path);
        // println!("{:?}", data_url);

        if let Some(time_info) = &time_info
            && let Some(gps_info) = &gps_info
        {
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

#[cfg(test)]
mod tests {
    use super::*;
    use exiftool::ExifTool;
    use media_analyzer::tags::TagData;
    use std::path::Path;

    /// Helper function to reduce boilerplate in tests.
    /// It takes a relative path to an asset, runs exiftool, and returns the extracted tags.
    fn get_tags_for_asset(relative_path: &str) -> color_eyre::Result<TagData> {
        // Assume tests run from the project root where the 'assets' dir is.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join(relative_path);

        if !path.exists() {
            panic!("Test asset file not found at: {:?}", path);
        }

        let mut et = ExifTool::new()?;
        let exif_data = et.json(&path, &["-n"])?;

        // println!("{}", serde_json::to_string_pretty(&exif_data).unwrap());

        Ok(extract_tags(&path, &exif_data))
    }

    #[test]
    fn test_night_sight_photo() {
        let tags = get_tags_for_asset("night_sight/PXL_20250104_170020532.NIGHT.jpg").unwrap();
        assert!(
            tags.is_night_sight,
            "Should be detected as Night Sight from filename"
        );
        assert!(!tags.is_video);
        assert!(!tags.is_motion_photo);
    }

    #[test]
    fn test_motion_photo() {
        let tags = get_tags_for_asset("motion/PXL_20250103_180944831.MP.jpg").unwrap();
        assert!(
            tags.is_motion_photo,
            "Should be detected as a Motion Photo from EXIF tag"
        );
        assert!(
            tags.motion_photo_presentation_timestamp.is_some(),
            "Should have a presentation timestamp"
        );
        // The primary file is an image, but it contains a video component.
        assert!(
            !tags.is_video,
            "Motion photos contain a video stream but is not a video."
        );
    }

    #[test]
    fn test_photosphere() {
        let tags = get_tags_for_asset("photosphere.jpg").unwrap();
        assert!(tags.is_photosphere, "Should be detected as a photosphere");
        assert!(tags.use_panorama_viewer, "Should require a panorama viewer");
        assert_eq!(
            tags.projection_type,
            Some("equirectangular".to_string()),
            "Projection type should be equirectangular"
        );
        assert!(!tags.is_video);
    }

    #[test]
    fn test_burst_photos() {
        // Test case 1: Google Pixel burst format
        let tags1 =
            get_tags_for_asset("burst/00000IMG_00000_BURST20201123164411530_COVER.jpg").unwrap();
        assert!(tags1.is_burst, "Should detect burst format 1");
        assert_eq!(
            tags1.burst_id,
            Some("00000img_00000".to_string()),
            "Should extract correct burst ID for format 1"
        );

        // Test case 2: Samsung/Older burst format
        let tags2 = get_tags_for_asset("burst/20150813_160421_Burst01.jpg").unwrap();
        assert!(tags2.is_burst, "Should detect burst format 2");
        assert_eq!(
            tags2.burst_id,
            Some("20150813_160421".to_string()),
            "Should extract correct burst ID for format 2"
        );
    }

    #[test]
    fn test_slow_motion_video() {
        let tags = get_tags_for_asset("slowmotion.mp4").unwrap();
        assert!(tags.is_video, "Should be detected as a video");
        assert!(tags.is_slowmotion, "Should be detected as slow motion");
        assert!(!tags.is_timelapse, "Should not be a timelapse");

        // This assertion is key for slow motion detection
        if let (Some(capture), Some(video)) = (tags.capture_fps, tags.video_fps) {
            assert!(
                capture > video,
                "Capture FPS ({}) must be greater than video FPS ({}) for slow motion",
                capture,
                video
            );
        } else {
            panic!("Capture FPS and Video FPS could not be determined for slow motion file.");
        }
    }

    #[test]
    fn test_timelapse_video() {
        let tags = get_tags_for_asset("timelapse.mp4").unwrap();
        assert!(tags.is_video, "Should be detected as a video");
        assert!(tags.is_timelapse, "Should be detected as a timelapse");
        assert!(!tags.is_slowmotion, "Should not be slow motion");
    }

    #[test]
    fn test_standard_video() {
        let tags = get_tags_for_asset("video/car.webm").unwrap();
        assert!(tags.is_video, "Should be detected as a video");
        assert!(!tags.is_slowmotion);
        assert!(!tags.is_timelapse);
        assert!(!tags.is_motion_photo);
    }

    #[test]
    fn test_standard_images() {
        let tags_jpg = get_tags_for_asset("sunset.jpg").unwrap();
        assert!(!tags_jpg.is_video, "Standard JPG should not be a video");
        assert!(
            !tags_jpg.is_burst,
            "Standard JPG should not be a burst photo"
        );
        assert!(
            !tags_jpg.is_night_sight,
            "Standard JPG should not be night sight"
        );
        assert!(
            !tags_jpg.is_photosphere,
            "Standard JPG should not be a photosphere"
        );

        let tags_png = get_tags_for_asset("png_image.png").unwrap();
        assert!(!tags_png.is_video, "PNG should not be a video");

        let tags_gif = get_tags_for_asset("cat_bee.gif").unwrap();
        assert!(!tags_gif.is_video, "GIF should not be a video");
    }

    #[test]
    fn test_non_media_file() {
        // This will get an empty JSON object from our robust helper function
        let tags = get_tags_for_asset("text_file.txt").unwrap();

        // Assert that all boolean flags are false and Options are None
        assert!(!tags.is_video);
        assert!(!tags.is_burst);
        assert!(!tags.is_hdr);
        assert!(!tags.is_motion_photo);
        assert!(!tags.is_night_sight);
        assert!(!tags.is_photosphere);
        assert!(!tags.is_slowmotion);
        assert!(!tags.is_timelapse);
        assert!(!tags.use_panorama_viewer);
        assert!(tags.burst_id.is_none());
        assert!(tags.projection_type.is_none());
        assert!(tags.capture_fps.is_none());
        assert!(tags.video_fps.is_none());
    }
}
