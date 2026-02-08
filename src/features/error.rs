use thiserror::Error;

#[derive(Error, Debug)]
pub enum MetadataError {
    #[error("Missing required metadata field: {0}")]
    MissingRequiredField(String),
}

#[derive(Error, Debug)]
pub enum DataUrlError {
    #[error("Unsupported file type for data URL generation: {0}")]
    UnsupportedFileType(String),

    // For thumbnail.write_to, though rare for in-memory buffers
    #[error("I/O error during thumbnail generation")]
    Io(#[from] std::io::Error),
}
