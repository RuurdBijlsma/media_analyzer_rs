# media_analyzer

[![Crates.io](https://img.shields.io/crates/v/media_analyzer.svg)](https://crates.io/crates/media_analyzer)
[![Docs.rs](https://docs.rs/media_analyzer/badge.svg)](https://docs.rs/media_analyzer)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE.md)
[![Repository](https://img.shields.io/badge/GitHub-Repo-blue)](https://github.com/RuurdBijlsma/media_analyzer_rs)
[![Build Status](https://github.com/RuurdBijlsma/media_analyzer_rs/actions/workflows/ci.yml/badge.svg)](https://github.com/RuurdBijlsma/media_analyzer_rs/actions/workflows/ci.yml)

A Rust crate for extracting information from video and photo files.

This crate provides a high-level, asynchronous API that acts as an orchestrator over tools like `exiftool`, combining
raw metadata with intelligent parsing, geolocation data to produce a single, structured, and
easy-to-use result.

Example output, converted to JSON,
viewable [here](https://github.com/RuurdBijlsma/media_analyzer_rs/blob/main/.github/example_output/example_output.json).

## Prerequisites

This crate requires an installation of **`exiftool`** to be available. If you don't want it in your PATH, you can pass
the location of the executable in when building a `MediaAnalyzer`.

* **Official Website & Installation:** [https://exiftool.org/](https://exiftool.org/)
* **macOS (Homebrew):** `brew install exiftool`
* **Debian/Ubuntu:** `sudo apt install libimage-exiftool-perl`
* **Windows:** Download the Windows Executable from the official website and ensure its location is in your PATH
  environment variable.

Verify your installation by typing `exiftool -ver` in your terminal.

## Features

* **Unified Metadata:** Gathers core properties (`FileMetadata`) and photographic details (`CaptureDetails`) from
  media files.
* **Time Resolution:** Analyzes multiple exif tags to determine the most accurate UTC timestamp and
  timezone (`TimeInfo`).
* **Geolocation:** Performs reverse geocoding on GPS coordinates (`GpsInfo`).
* **Rich Media Tagging:** Identifies special characteristics like `is_motion_photo`, `is_hdr`, `is_burst`, and
  `is_slowmotion` (`TagData`).

## The `MediaMetadata` Struct

The primary output of this crate is the `MediaMetadata` struct. It is a single, consolidated container that holds all
the information gathered during the analysis pipeline, making it easy to access any piece of data you need.

## Installation

Add `media_analyzer` to your `Cargo.toml` dependencies:

```bash
cargo add media_analyzer
```

## Quick Start

Create a `MediaAnalyzer` instance using its builder, then call the `analyze_media` method.

```rust
use media_analyzer::{MediaAnalyzer, MediaAnalyzerError};

#[tokio::main]
async fn main() -> Result<(), MediaAnalyzerError> {
    // 1. Build the analyzer. The builder allows for custom configuration.
    let analyzer = MediaAnalyzer::builder()
        .build()?;

    // 2. Define the path to the photo or video file to analyze.
    let media_file = Path::new("path/to/your/photo.jpg");

    // 3. Analyze the photo, for a video you'd use analyze_media.
    //    The thumbnail should be tiny, as it's meant to be a preview file 
    //    that might be shown while the real image is loading. For example, at most 10x10 pixels.
    let result = analyzer.analyze_media(media_file).await?;

    // 4. Access the data from the `MediaMetadata`.
    if let Some(gps) = result.gps_info {
        println!("Location: {}, {}", gps.location.name, gps.location.country_code);
    }

    if let Some(model) = result.capture_details.camera_model {
        println!("Camera: {}", model);
    }

    if let Some(utc_time) = result.time_info.datetime_utc {
        println!("Taken at (UTC): {}", utc_time);
    }

    if result.tags.is_hdr {
        println!("This is an HDR image.");
    }

    Ok(())
}
```

## Error Handling

All potentially failing operations return `Result<_, MediaAnalyzerError>`. The `MediaAnalyzerError` enum covers
critical failures in the analysis pipeline, including:

* `Exiftool`: The `exiftool` process failed to execute or read the file.
* `Metadata`: The media file was missing essential tags required for analysis (e.g., `ImageWidth`).
* `Time`: No usable time or date information could be extracted from any source.
* `DataUrl`: The provided thumbnail path was invalid or not a supported image format.
* And others for I/O and initialization issues.

## Core Dependencies

This crate is a high-level orchestrator that builds upon several powerful tools and libraries:

* **[ExifTool](https://exiftool.org/)**: The definitive tool for reading and writing media metadata.
* **[exiftool_rs](https://crates.io/crates/exiftool)**: For persistent communication with the `exiftool` process.
* **[reverse_geocoder](https://crates.io/crates/reverse_geocoder)**: For offline reverse geocoding.
* **[Chrono](https://crates.io/crates/chrono)** & **[Chrono-tz](https://crates.io/crates/chrono-tz)**: For time and
  timezone handling.

## API Documentation

Full API documentation is available on [docs.rs](https://docs.rs/media_analyzer).

## Contributing

Contributions, bug reports, and feature requests are welcome! Please open an issue or submit a pull request on
the [GitHub repository](https://github.com/RuurdBijlsma/media_analyzer_rs).

## License

This crate is licensed under the Apache License 2.0. See the `LICENSE.md` file for details.