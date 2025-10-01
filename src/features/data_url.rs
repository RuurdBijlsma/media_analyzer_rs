use crate::features::error::DataUrlError;
use base64::{Engine as _, engine::general_purpose};
use mime_guess::MimeGuess;
use std::path::Path;
use tokio::fs;

pub async fn file_to_data_url<P: AsRef<Path>>(path: P) -> Result<String, DataUrlError> {
    let mime = MimeGuess::from_path(&path).first_or_octet_stream();
    let bytes = fs::read(&path).await?;
    let b64 = general_purpose::STANDARD.encode(bytes);
    let data_url = format!("data:{};base64,{}", mime.essence_str(), b64);
    Ok(data_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Import the specific error enum for this module
    use crate::features::error::DataUrlError;
    use std::path::Path;

    #[tokio::test]
    async fn test_generates_data_url_for_valid_jpg() -> Result<(), DataUrlError> {
        // Use the standard JPEG file as the primary success case
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("sunset.jpg");

        // The test should panic if this fails, so .unwrap() is appropriate here.
        let data_url = file_to_data_url(&path).await?;

        assert!(
            data_url.starts_with("data:image/jpeg;base64,"),
            "Data URL should have the correct JPEG MIME type prefix"
        );
        assert!(
            data_url.len() > "data:image/jpeg;base64,".len(),
            "Data URL should contain Base64 data"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_handles_png_input_correctly() -> Result<(), DataUrlError> {
        // Ensure it correctly processes a PNG and converts it to a JPEG data URL
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("png_image.png");

        let result = file_to_data_url(&path).await;
        assert!(result.is_ok(), "Should successfully process a PNG file");

        Ok(())
    }

    #[tokio::test]
    async fn test_handles_avif_input_correctly() -> Result<(), DataUrlError> {
        // Ensure it correctly processes a PNG and converts it to a JPEG data URL
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("thumbnail-small.avif");

        let result = file_to_data_url(&path).await;
        assert!(result.is_ok(), "Should successfully process a avif file");

        Ok(())
    }
}
