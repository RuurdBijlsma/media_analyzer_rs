use base64::{engine::general_purpose, Engine as _};
use color_eyre::eyre;
use image::ImageFormat;
use mime_guess::MimeGuess;
use std::io::Cursor;
use std::path::Path;

pub fn file_to_data_url<P: AsRef<Path>>(path: P) -> color_eyre::Result<String> {
    let path = path.as_ref();
    let mime = MimeGuess::from_path(path).first_or_octet_stream();

    // Check if the guessed MIME type is an image.
    if mime.type_() == "image" {
        // --- NEW LOGIC FOR IMAGES ---

        // 1. Open the image file using the `image` crate.
        //    This can fail if the file is not a supported image format.
        let img = image::open(path)?;

        // 2. Create a thumbnail. This resizes the image to fit within
        //    a 100x100 box, maintaining the aspect ratio.
        let thumbnail = img.thumbnail(10, 10);

        // 3. We need to write the resized image data to an in-memory buffer.
        //    A Cursor wrapping a Vec<u8> is perfect for this.
        let mut bytes = Cursor::new(Vec::new());

        // 4. Write the thumbnail into the buffer as a JPEG.
        //    JPEG is a good choice for small previews. An error here would be an I/O error.
        thumbnail.write_to(&mut bytes, ImageFormat::Jpeg)?;

        // 5. Encode the bytes of the *newly created thumbnail* into Base64.
        let b64 = general_purpose::STANDARD.encode(bytes.into_inner());

        // 6. Format the data URL. Since we encoded it as a JPEG, we use "image/jpeg".
        let data_url = format!("data:image/jpeg;base64,{}", b64);
        Ok(data_url)
    } else {
        eyre::bail!("Unsupported file type")
    }
}
