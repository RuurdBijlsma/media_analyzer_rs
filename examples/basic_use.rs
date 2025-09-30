use media_analyzer::media_analyzer::MediaAnalyzer;
use std::path::Path;
// TODO: make rust package
// add error handling
// Reverse geolocation
// add gps accuracy to gpsInfo
// add bearing/movement speed to gpsInfo

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let path = Path::new("assets/sunset.jpg");
    let mut analyzer = MediaAnalyzer::builder().build().await?;
    let analyze_result = analyzer.analyze_media(path, vec![path]).await?;
    println!("{}", serde_json::to_string_pretty(&analyze_result)?);

    Ok(())
}
