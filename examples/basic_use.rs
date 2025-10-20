use media_analyzer::{MediaAnalyzer, MediaAnalyzerError};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), MediaAnalyzerError> {
    let path = Path::new("assets/hdr.jpg");
    let mut analyzer = MediaAnalyzer::builder().build().await?;
    let analyze_result = analyzer.analyze_media(path).await?;
    println!("{}", serde_json::to_string_pretty(&analyze_result).unwrap());

    Ok(())
}
