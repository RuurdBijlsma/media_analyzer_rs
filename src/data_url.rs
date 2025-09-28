use base64::{engine::general_purpose, Engine as _};
use mime_guess::MimeGuess;
use std::fs;
use std::path::Path;
// so you can guess the media type

pub fn file_to_data_url<P: AsRef<Path>>(path: P) -> color_eyre::Result<String> {
    // todo make image small first?
    let path = path.as_ref();
    let data = fs::read(path)?;
    let b64 = general_purpose::STANDARD.encode(&data);
    let joy = MimeGuess::from_path(path).first_or_octet_stream();

    // guess mime type from extension
    let mime_type = joy.essence_str();

    let data_url = format!("data:{};base64,{}", mime_type, b64);
    Ok(data_url)
}
