# media_analyzer

[![Crates.io](https://img.shields.io/crates/v/media_analyzer.svg)](https://crates.io/crates/media_analyzer)
[![Docs.rs](https://docs.rs/media_analyzer/badge.svg)](https://docs.rs/media_analyzer)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE.md)
[![Repository](https://img.shields.io/badge/GitHub-Repo-blue)](https://github.com/RuurdBijlsma/media_analyzer_rs)
[![Build Status](https://github.com/RuurdBijlsma/media_analyzer_rs/actions/workflows/ci.yml/badge.svg)](https://github.com/RuurdBijlsma/media_analyzer_rs/actions/workflows/ci.yml)

`media_analyzer` can extract structured metadata, resolved timestamps, timezone offsets, offline geolocation, and
historical weather details from photo and video files.

---

## Prerequisites

This crate requires the [ExifTool](https://exiftool.org/) command-line utility. Ensure the executable is available in
your system's `PATH`, or specify its path directly when configuring the analyzer.

---

## Features

* **Unified Metadata**: Normalizes dimensions, format, duration, and orientation across photos and videos, alongside
  camera details (such as ISO, aperture, exposure time, and lens model).
* **Time & Timezone Resolution**: Evaluates EXIF tags, filesystem dates, and filename pattern fallbacks to resolve UTC
  times, local times, and timezone offsets.
* **Offline Geolocation**: Maps GPS coordinates to cities, regions, and countries without requiring a network
  connection.
* **Weather & Sun Alignment**: Retrieves historical weather data and calculates solar events (such as sunrise or sunset)
  matching the time and place of capture.
* **Smart Media Tagging**: Detects properties like HDR, motion photos, slow-motion capture rates, burst sequences, and
  timelapses.

---

## Usage Example

Add `media_analyzer` to your `Cargo.toml`:

```toml
[dependencies]
media_analyzer = "0.10.3"
```

```rust
use std::path::Path;
use media_analyzer::{MediaAnalyzer, MediaAnalyzerError};

#[tokio::main]
async fn main() -> Result<(), MediaAnalyzerError> {
    let analyzer = MediaAnalyzer::builder().build().await?;
    let result = analyzer.analyze_media(Path::new("assets/sunset.jpg")).await?;
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    // `result` is of type: `MediaMetadata`
    Ok(())
}
```

For a view of the output returned by the analyzer, see
the [example output JSON](.github/example_output/example_output.json).

---

## Custom ExifTool Binary

If ExifTool is installed outside your system `PATH`, you can specify its executable path directly during setup:

```rust
use std::path::Path;
use media_analyzer::MediaAnalyzer;

async fn init_custom() -> Result<MediaAnalyzer, media_analyzer::MediaAnalyzerError> {
    MediaAnalyzer::builder()
        .exiftool_path(Some(Path::new("/custom/path/to/exiftool")))
        .build()
        .await
}
```