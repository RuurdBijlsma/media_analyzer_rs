use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum TimeError {
    #[error("Could not extract any usable time metadata from the file")]
    Extraction,
}
