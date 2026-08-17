use base64::Engine;
use criterion::{Criterion, criterion_group, criterion_main};
use html_to_img::{RenderConfig, render_html_to_png};

const SIMPLE_HTML: &str = r#"<html><body style="background: #ffffff; font-family: sans-serif; padding: 40px;"><h1>Benchmark</h1><p>Some paragraph text that wraps across a few lines to exercise layout.</p></body></html>"#;

fn html_with_image() -> String {
    let mut cursor = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(64, 64, image::Rgba([120, 40, 200, 255]))
        .write_to(&mut cursor, image::ImageFormat::Png)
        .unwrap();
    let b64 = base64::engine::general_purpose::STANDARD.encode(cursor.into_inner());
    format!(
        r#"<html><body style="background: #ffffff"><img src="data:image/png;base64,{b64}" width="200" height="200"></body></html>"#
    )
}

fn bench(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = RenderConfig::default();
    let image_html = html_with_image();

    c.bench_function("render_simple_html", |b| {
        b.iter(|| {
            let cfg = config.clone();
            rt.block_on(async {
                tokio::task::spawn_blocking(move || render_html_to_png(SIMPLE_HTML, &cfg))
                    .await
                    .unwrap()
                    .unwrap()
            })
        });
    });

    c.bench_function("render_html_with_image", |b| {
        b.iter(|| {
            let cfg = config.clone();
            let html = image_html.clone();
            rt.block_on(async {
                tokio::task::spawn_blocking(move || render_html_to_png(&html, &cfg))
                    .await
                    .unwrap()
                    .unwrap()
            })
        });
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
