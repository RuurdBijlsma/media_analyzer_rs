#![allow(clippy::too_many_lines, clippy::cast_possible_truncation)]
//! # Media Analyzer
//!
//! A toolkit for extracting info from video and photo files.
//!
//! This crate provides a high-level, asynchronous API to analyze media files. It acts as a
//! facade over tools like `exiftool`, combining raw metadata with parsing,
//! geolocation, and historical weather data to produce a single, easy-to-use result.
//!
//! The core philosophy is to be a "best-effort" analyzer. It robustly processes what it can,
//! and provides detailed information in a structured format.
//!
//! ## Prerequisites
//!
//! This crate requires a command-line installation of **`exiftool`** to be available in the
//! system's PATH. You can download it from the [official ExifTool website](https://exiftool.org/).
//! You can also pass the location of you exiftool executable if you don't want it in PATH.
//!
//! ## Key Features
//!
//! - **Unified Metadata**: Gathers basic properties like width, height, duration, and MIME type
//!   into a clean [`FileMetadata`] struct, while also providing photographic details like ISO,
//!   aperture, and camera model in [`CaptureDetails`].
//!
//! - **Time Resolution**: It analyzes multiple EXIF
//!   tags, file metadata, and GPS data to determine the most accurate UTC timestamp and timezone
//!   information, summarized in the [`TimeInfo`] struct.
//!
//! - **Geolocation & Weather**: Automatically performs reverse geocoding on GPS coordinates to find
//!   human-readable location names ([`GpsInfo`]). If successful, it then fetches historical weather
//!   and sun data (sunrise, sunset) for the precise time and place the media was captured,
//!   populating the [`WeatherInfo`] struct.
//!
//! - **Rich Media Tagging**: Identifies a wide variety of special media characteristics, such as
//!   `is_motion_photo`, `is_hdr`, `is_burst`, `is_slowmotion`, and `is_timelapse`, all available
//!   in the [`TagData`] struct.
//!
//! - **Thumbnail Generation**: Creates a tiny, Base64-encoded JPEG data URL, for use as
//!   a blurred placeholder in a UI while the full media loads.
//!
//! ## The `AnalyzeResult` Struct
//!
//! The primary output of this crate is the [`AnalyzeResult`] struct. It is a single, consolidated
//! container that holds all the information gathered during the analysis pipeline, making it
//! easy to access any piece of data you need.
//!
//! ## Usage
//!
//! 1.  Create a [`MediaAnalyzer`] instance using its builder.
//! 2.  Call the [`MediaAnalyzer::analyze_media`] method with the path to your media file.
//!
//! ```rust
//! use std::path::Path;
//! use media_analyzer::{MediaAnalyzer, MediaAnalyzerError};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), MediaAnalyzerError> {
//!     // 1. Build the analyzer. The builder allows for custom configuration.
//!     let mut analyzer = MediaAnalyzer::builder()
//!         .weather_search_radius_km(50.0) // Optional: configure the analyzer
//!         .build()
//!         .await?;
//!
//!     // 2. Define the path to the media file to analyze.
//!     let media_file = Path::new("assets/sunset.jpg");
//!
//!     // 3. Analyze the media file. For a photo, the file itself can serve as the thumbnail.
//!     let result = analyzer.analyze_media(media_file).await?;
//!
//!     // 4. Access the structured data from the `AnalyzeResult`.
//!     if let Some(gps) = result.gps_info {
//!         println!("Location: {}, {}", gps.location.name, gps.location.country_code);
//!     }
//!
//!     if let Some(model) = result.capture_details.camera_model {
//!         println!("Camera: {}", model);
//!     }
//!
//!     if let Some(utc_time) = result.time_info.datetime_utc {
//!         println!("Taken at (UTC): {}", utc_time);
//!     }
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

// The main result struct and its components
pub use features::gps::{GpsInfo, LocationName};
pub use features::metadata::{CaptureDetails, FileMetadata};
pub use features::pano::{PanoInfo, PanoViewInfo};
pub use features::weather::{SunInfo, WeatherInfo};
pub use structs::AnalyzeResult;
pub use tags::structs::TagData;
pub use time::structs::{SourceDetails, TimeInfo, TimeZoneInfo};
