use crate::features::error::DataUrlError;
use base64::{Engine as _, engine::general_purpose};
use image::ImageFormat;
use mime_guess::MimeGuess;
use std::io::Cursor;
use std::path::Path;

pub fn file_to_data_url<P: AsRef<Path>>(path: P) -> Result<String, DataUrlError> {
    let path = path.as_ref();
    let mime = MimeGuess::from_path(path).first_or_octet_stream();

    if mime.type_() != "image" {
        // Return our specific error variant
        return Err(DataUrlError::UnsupportedFileType(mime.to_string()));
    }

    // The '?' operator will now work with #[from] to convert errors
    let img = image::open(path)?;
    let thumbnail = img.thumbnail(10, 10);
    let mut bytes = Cursor::new(Vec::new());
    thumbnail.write_to(&mut bytes, ImageFormat::Jpeg)?;
    let b64 = general_purpose::STANDARD.encode(bytes.into_inner());
    let data_url = format!("data:image/jpeg;base64,{}", b64);
    Ok(data_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_file_to_data_url_with_valid_image() -> color_eyre::Result<()> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("png_image.png");

        let data_url_result = file_to_data_url(&path);
        assert!(
            data_url_result.is_ok(),
            "Should successfully process a valid image"
        );

        let data_url = data_url_result?;
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

    #[test]
    fn test_file_to_data_url_with_invalid_file() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("text_file.txt");

        let data_url_result = file_to_data_url(&path);
        assert!(
            data_url_result.is_err(),
            "Should return an error for non-image files"
        );
    }

    #[test]
    fn test_file_to_data_url_with_corrupted_image() {
        // This test assumes 'invalid_image.png' is a file that is not a valid png,
        // for example, a text file renamed to .png
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("invalid_image.png");

        let data_url_result = file_to_data_url(&path);
        assert!(
            data_url_result.is_err(),
            "Should return an error for corrupted or invalid image files"
        );
    }
}
