use std::io;
use std::path::Path;

/// Computes the BLAKE3 hash of a file.
pub fn hash_file(path: &Path) -> io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_mmap_rayon(path)?;

    let hash = hasher.finalize();
    Ok(hash.to_hex().to_string())
}
