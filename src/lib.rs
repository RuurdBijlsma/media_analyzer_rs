//! # Media Analyzer
//!
//! Extract a wide range of information from video and photo files.
//!
//! This crate provides a high-level API to analyze media files and retrieve detailed
//! metadata, including Exif data, geolocation, time of capture, weather conditions,
//! and more.
//!
//! ## Key Features
//!
//! - **Exif Data**: Extracts raw Exif information from media files.
//! - **Time Information**: Extracts the time a photo or video was taken, including timezone data.
//! - **GPS Location**: Retrieves GPS coordinates and uses them to find the corresponding location name.
//! - **Weather Conditions**: Fetches historical weather data for the specific time and place of media capture, including sunrise and sunset times.
//! - **Specialized Tags**: Identifies unique tags such as whether the media is a motion photo, a panorama, or was taken with night sight mode.
//! - **Basic Media Info**: Gathers fundamental properties like width, height, duration, and MIME type.
//!
//! ## Usage
//!
//! To get started, create an instance of `MediaAnalyzer` and then call the `analyze_media` method with the path to your media file.
//!
//! ```rust,no_run
//! use std::path::Path;
//! use media_analyzer::MediaAnalyzer;
//! use media_analyzer::MediaAnalyzerError;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), MediaAnalyzerError> {
//!     // Path to the media file you want to analyze.
//!     let media_file = Path::new("assets/sunset.jpg");
//!
//!     // Analyze the media file.
//!     let mut analyzer = MediaAnalyzer::builder().build().await?;
//!     let result = analyzer.analyze_media(media_file, media_file).await?;
//!
//!     // Print the extracted information.
//!     println!("GPS Location: {:?}", result.gps_info);
//!     println!("Weather: {:?}", result.weather_info);
//!     println!("Tags: {:?}", result.tags);
//!
//!     Ok(())
//! }
//! ```

mod error;
mod features;
mod media_analyzer;
mod structs;
mod tags;
mod time;

// --- Public API Exports ---
pub use media_analyzer::MediaAnalyzer;
pub use media_analyzer::MediaAnalyzerBuilder;

// The primary error type
pub use error::MediaAnalyzerError;

// The data structures for easy access
pub use features::gps::GpsInfo;
pub use features::metadata::{CaptureDetails, FileMetadata};
pub use features::pano::{PanoInfo, PanoViewInfo};
pub use features::weather::{SunInfo, WeatherInfo};
pub use structs::AnalyzeResult;
pub use tags::structs::TagData;
pub use time::structs::TimeInfo;
