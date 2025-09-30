use criterion::{criterion_group, criterion_main, Criterion};
use media_analyzer::media_analyzer::MediaAnalyzer;
use std::path::Path;
use tokio::runtime::Runtime;

fn bench(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("media_analyzer::new", |b| {
        b.iter(|| {
            rt.block_on(async {
                MediaAnalyzer::builder().build().await.unwrap();
            });
        });
    });

    let mut media_analyzer = rt.block_on(async { MediaAnalyzer::builder().build().await.unwrap() });
    let image_path = Path::new("./assets/tent.jpg");

    c.bench_function("media_analyzer.analyze_media", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = media_analyzer
                    .analyze_media(image_path, vec![image_path])
                    .await
                    .unwrap();
            });
        });
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
