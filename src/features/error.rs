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

    #[error("Failed to process image")]
    ImageProcessing(#[from] image::ImageError),

    // For thumbnail.write_to, though rare for in-memory buffers
    #[error("I/O error during thumbnail generation")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum WeatherError {
    #[error("Weather API call failed")]
    ApiError(#[from] meteostat::MeteostatError),

    #[error("No weather data available for the specified time and location")]
    NoDataAvailable,

    #[error("Failed to calculate sun position")]
    SunCalculationError,
}
