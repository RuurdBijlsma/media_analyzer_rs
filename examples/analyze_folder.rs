use media_analyzer::{MediaAnalyzer, MediaAnalyzerError};
use rand::prelude::IndexedRandom;
use rand::rng;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

/// Checks if a directory entry is hidden (starts with '.').
fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .is_some_and(|s| s.starts_with('.'))
}

/// Recursively lists all files using `walkdir` and `filter_entry`.
/// This version propagates I/O errors encountered during traversal.
/// # Errors
/// * Error on read entry
pub fn list_files_walkdir_filtered(
    dir: &Path,
    include_hidden: bool,
) -> Result<Vec<PathBuf>, walkdir::Error> {
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| if include_hidden { true } else { !is_hidden(e) })
        .filter_map(|entry_result| match entry_result {
            Ok(entry) => {
                if entry.file_type().is_file() {
                    Some(Ok(entry.path().to_path_buf()))
                } else {
                    None
                }
            }
            Err(e) => Some(Err(e)),
        })
        .collect()
}

/// Sample random photo(s) from a folder to test the media analyzer for various files.
#[tokio::main]
async fn main() -> Result<(), MediaAnalyzerError> {
    let mut analyzer = MediaAnalyzer::builder().build().await?;

    let start_dir = Path::new("E:/Backup/Photos/photos/photos");
    let all_files = list_files_walkdir_filtered(start_dir, false).unwrap();
    let sample_size = 1;
    let mut rng_machine = rng();
    let sampled_files_iter = all_files.sample(&mut rng_machine, sample_size.min(all_files.len()));

    // Iterate over the sampled files
    for file in sampled_files_iter {
        let path = &file.canonicalize()?;
        opener::open(path).expect("can't open photo");
        println!("\t{}", path.display());
        let analyze_result = analyzer.analyze_media(path).await?;
        println!("{}", serde_json::to_string_pretty(&analyze_result).unwrap());
    }

    Ok(())
}
