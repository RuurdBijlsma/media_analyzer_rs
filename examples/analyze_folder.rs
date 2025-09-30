use media_analyzer::media_analyzer::MediaAnalyzer;
use media_analyzer::utils::list_files_walkdir_filtered;
use rand::prelude::IndexedRandom;
use rand::rng;
use std::path::Path;

/// Sample random photo(s) from a folder to test the media analyzer for various files.
#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let mut analyzer = MediaAnalyzer::builder().build().await?;

    let start_dir = Path::new("E:/Backup/Photos/photos/photos");
    let all_files = list_files_walkdir_filtered(start_dir, false)?;
    let sample_size = 1;
    let mut rng_machine = rng();
    let sampled_files_iter =
        all_files.choose_multiple(&mut rng_machine, sample_size.min(all_files.len()));

    // Iterate over the sampled files
    for file in sampled_files_iter {
        let path = &file.canonicalize()?;
        opener::open(path).expect("can't open photo");
        println!("\t{}", path.display());
        let analyze_result = analyzer.analyze_media(path, vec![path]).await?;
        println!("{}", serde_json::to_string_pretty(&analyze_result)?);
    }

    Ok(())
}
