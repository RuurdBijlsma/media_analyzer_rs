use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

/// Checks if a directory entry is hidden (starts with '.').
fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Recursively lists all files using `walkdir` and `filter_entry`.
/// This version propagates I/O errors encountered during traversal.
pub fn list_files_walkdir_filtered(dir: &Path, include_hidden: bool) -> Result<Vec<PathBuf>, walkdir::Error> {
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| { // Filter entries *before* processing/descending
            if include_hidden {
                true // Always include if include_hidden is true
            } else {
                !is_hidden(e) // Include only if not hidden
            }
        })
        .filter_map(|entry_result| { // Process results after filtering
            // Instead of entry_result.ok(), handle the Result explicitly
            match entry_result {
                Ok(entry) => {
                    // If the entry is Ok, check if it's a file
                    if entry.file_type().is_file() {
                        // If it's a file, wrap its path in Some(Ok(...))
                        // Some -> keep it after filter_map
                        // Ok   -> indicates success for this item for collect::<Result>
                        Some(Ok(entry.path().to_path_buf()))
                    } else {
                        // If it's a directory, filter it out (return None)
                        None
                    }
                }
                Err(e) => {
                    // If reading the entry failed, propagate the error
                    // Wrap the Err in Some(...) so filter_map keeps it
                    // collect::<Result> will see this Err and stop, returning it.
                    Some(Err(e))
                }
            }
        })
        // Now the iterator yields Result<PathBuf, walkdir::Error> items.
        // Collect can correctly gather these into a Result<Vec<PathBuf>, walkdir::Error>.
        .collect()
}
