use thiserror::Error;

#[derive(Error, Debug)]
pub enum MediaAnalyzerError {
    #[error("Exiftool failed to execute or process the file")]
    Exiftool(#[from] exiftool::ExifToolError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Time extraction failed: {0}")]
    Time(#[from] crate::time::error::TimeError),

    #[error("Essential metadata extraction failed: {0}")]
    Metadata(#[from] crate::features::error::MetadataError),

    #[error("Data URL generation failed: {0}")]
    DataUrl(#[from] crate::features::error::DataUrlError),

    #[error("No thumbnail frames were provided to generate a data URL")]
    NoThumbnail,
}
