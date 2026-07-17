use crate::ExifData;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Checks if a companion file exists (e.g. .mov, .MOV, .mp4, .MP4).
fn check_companion_files(input_file: &Path) -> bool {
    let input_ext = input_file
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase);

    let companion_extensions = ["mov", "MOV", "mp4", "MP4"];
    for ext in &companion_extensions {
        // Skip checking if the companion extension matches the current file's extension case-insensitively.
        if let Some(ref iext) = input_ext
            && iext == &ext.to_lowercase()
        {
            continue;
        }

        let companion_path = input_file.with_extension(ext);
        if companion_path.exists() && companion_path.is_file() {
            return true;
        }
    }
    false
}

/// Checks if a slice looks like a valid MP4/MOV video.
fn is_valid_video(bytes: &[u8], check_length: bool) -> bool {
    if check_length && bytes.len() < 1000 {
        return false;
    }
    let first_chunk = if bytes.len() > 64 {
        &bytes[..64]
    } else {
        bytes
    };
    first_chunk
        .windows(4)
        .any(|w| w == b"ftyp" || w == b"mdat" || w == b"moov")
}

/// Scans forward to find the MP4 start offset.
fn find_embedded_mp4_start(data: &[u8]) -> Option<usize> {
    if data.len() < 8 {
        return None;
    }

    let mut i = 0;
    let end = data.len() - 7;
    while i < end {
        // Find the next occurrence of b'f' first for high-performance scanning
        if data[i] == b'f' && &data[i..i + 4] == b"ftyp" {
            let brand = &data[i + 4..i + 8];
            if brand.iter().all(|&b| b.is_ascii_alphanumeric()) && i >= 4 {
                return Some(i - 4);
            }
        }
        i += 1;
    }
    None
}

/// Determines if the file has an embedded motion photo video.
pub fn detect_motion_photo(input_file: &Path, exif: &ExifData) -> bool {
    if exif.is_video() {
        return false;
    }
    if check_companion_files(input_file) {
        return true;
    }
    if exif.get_ignoring_case("MotionPhotoVideo").is_some()
        || exif.get_ignoring_case("EmbeddedVideoFile").is_some()
    {
        return true;
    }
    if let Some(offset_val) = exif.get_u64_ignoring_case("MicroVideoOffset")
        && offset_val > 0
        && let Ok(metadata) = std::fs::metadata(input_file)
    {
        let file_size = metadata.len();
        if file_size > offset_val
            && let Ok(mut f) = File::open(input_file)
        {
            let start_offset = file_size - offset_val;
            if f.seek(SeekFrom::Start(start_offset)).is_ok() {
                let mut buf = [0u8; 64];
                if f.read_exact(&mut buf).is_ok() && is_valid_video(&buf, false) {
                    return true;
                }
            }
        }
    }

    // If a JPEG ends with 0xFF 0xD9 (End of Image) near the end, there is no appended MP4.
    let is_jpeg = input_file
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            let ext_lower = ext.to_ascii_lowercase();
            ext_lower == "jpg" || ext_lower == "jpeg"
        });

    if is_jpeg {
        if let Ok(mut f) = File::open(input_file)
            && let Ok(metadata) = f.metadata()
        {
            let len = metadata.len();
            if len > 128 {
                let mut last_bytes = vec![0u8; 128];
                if f.seek(SeekFrom::End(-128)).is_ok()
                    && f.read_exact(&mut last_bytes).is_ok()
                    && let Some(pos) = last_bytes.windows(2).rposition(|w| w == [0xFF, 0xD9])
                {
                    // If EOI marker is within the last 32 bytes of the file,
                    // there is definitely no trailing MP4 appended.
                    if pos >= 96 {
                        return false;
                    }
                }
            }
        }

        // Fallback look for mp4
        if let Ok(data) = std::fs::read(input_file)
            && let Some(mp4_start_offset) = find_embedded_mp4_start(&data)
        {
            let video_bytes = &data[mp4_start_offset..];
            if is_valid_video(video_bytes, true) {
                return true;
            }
        }
    }

    false
}
