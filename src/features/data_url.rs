use crate::features::error::DataUrlError;
use base64::{Engine as _, engine::general_purpose};
use image::ImageFormat;
use mime_guess::MimeGuess;
use std::io::Cursor;
use std::path::Path;

pub fn file_to_data_url<P: AsRef<Path>>(
    path: P,
    thumbnail_max_size: (u32, u32),
) -> Result<String, DataUrlError> {
    let path = path.as_ref();
    let mime = MimeGuess::from_path(path).first_or_octet_stream();

    if mime.type_() != "image" {
        // Return our specific error variant
        return Err(DataUrlError::UnsupportedFileType(mime.to_string()));
    }

    // The '?' operator will now work with #[from] to convert errors
    let img = image::open(path)?;
    let thumbnail = img.thumbnail(thumbnail_max_size.0, thumbnail_max_size.1);
    let mut bytes = Cursor::new(Vec::new());
    thumbnail.write_to(&mut bytes, ImageFormat::Jpeg)?;
    let b64 = general_purpose::STANDARD.encode(bytes.into_inner());
    let data_url = format!("data:image/jpeg;base64,{}", b64);
    Ok(data_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    // Import the specific error enum for this module
    use crate::features::error::DataUrlError;
    use std::path::Path;

    #[test]
    fn test_generates_data_url_for_valid_jpg() {
        // Use the standard JPEG file as the primary success case
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("sunset.jpg");

        // The test should panic if this fails, so .unwrap() is appropriate here.
        let data_url = file_to_data_url(&path, (10, 10)).unwrap();

        assert!(
            data_url.starts_with("data:image/jpeg;base64,"),
            "Data URL should have the correct JPEG MIME type prefix"
        );
        assert!(
            data_url.len() > "data:image/jpeg;base64,".len(),
            "Data URL should contain Base64 data"
        );
    }

    #[test]
    fn test_handles_png_input_correctly() {
        // Ensure it correctly processes a PNG and converts it to a JPEG data URL
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("png_image.png");

        let result = file_to_data_url(&path, (20, 20));
        assert!(result.is_ok(), "Should successfully process a PNG file");
    }

    #[test]
    fn test_errs_on_non_image_file_with_correct_error_type() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("text_file.txt");

        let result = file_to_data_url(&path, (10, 10));

        // Assert that we got an error
        assert!(result.is_err(), "Should fail for a text file");

        // Assert that the error is the *specific variant* we expect.
        // This is a much stronger test than just checking is_err().
        assert!(
            matches!(result.unwrap_err(), DataUrlError::UnsupportedFileType(_)),
            "Error variant should be UnsupportedFileType"
        );
    }

    #[test]
    fn test_errs_on_corrupted_image_with_correct_error_type() {
        // 'invalid_image.png' is a text file renamed to .png
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join("invalid_image.png");

        let result = file_to_data_url(&path, (10, 10));

        // Assert that we got an error
        assert!(result.is_err(), "Should fail for a corrupted image");

        // Assert that the error is from the underlying 'image' crate,
        // correctly wrapped in our ImageProcessing variant.
        assert!(
            matches!(result.unwrap_err(), DataUrlError::ImageProcessing(_)),
            "Error variant should be ImageProcessing"
        );
    }
}