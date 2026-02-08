use criterion::{Criterion, criterion_group, criterion_main};
use media_analyzer::MediaAnalyzer;
use std::hint::black_box;
use std::path::Path;
use tokio::runtime::Runtime;

fn bench(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("media_analyzer::new", |b| {
        b.iter(|| {
            rt.block_on(async {
                MediaAnalyzer::builder().build().unwrap();
            });
        });
    });

    let media_analyzer = rt.block_on(async { MediaAnalyzer::builder().build().unwrap() });
    let image_path = Path::new("./assets/tent.jpg");

    c.bench_function("media_analyzer.analyze_media", |b| {
        b.iter(|| {
            rt.block_on(async {
                let _ = media_analyzer.analyze_media(black_box(image_path)).unwrap();
            });
        });
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
