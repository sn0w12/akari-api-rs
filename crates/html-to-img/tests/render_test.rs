use base64::Engine;
use html_to_img::render::{RenderConfig, render_html_to_buffer};
use html_to_img::svg::{rasterize_svg, svg_data_uri};

fn render(html: &str) -> html_to_img::render::RenderedImage {
    let html = html.to_string();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        tokio::task::spawn_blocking(move || render_html_to_buffer(&html, &RenderConfig::default()))
            .await
            .unwrap()
            .unwrap()
    })
}

fn red_png() -> Vec<u8> {
    use image::RgbaImage;
    let img = RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255]));
    let mut cursor = std::io::Cursor::new(Vec::new());
    img.write_to(&mut cursor, image::ImageFormat::Png).unwrap();
    cursor.into_inner()
}

fn pixel_at(img: &html_to_img::render::RenderedImage, x: u32, y: u32) -> [u8; 4] {
    let i = ((y as usize) * (img.width as usize) + x as usize) * 4;
    [
        img.rgba[i],
        img.rgba[i + 1],
        img.rgba[i + 2],
        img.rgba[i + 3],
    ]
}

#[test]
fn renders_a_simple_document() {
    let img = render(r#"<html><body style="background: #ffffff"><h1>Hi</h1></body></html>"#);

    assert_eq!((img.width, img.height), (1200, 630));
    // The body background fills the viewport.
    assert_eq!(pixel_at(&img, 0, 0), [255, 255, 255, 255]);
    assert_eq!(pixel_at(&img, 1199, 629), [255, 255, 255, 255]);
}

#[test]
fn renders_a_data_uri_image() {
    let b64 = base64::engine::general_purpose::STANDARD.encode(red_png());
    let img = render(&format!(
        r#"<html><body style="background: #ffffff"><img src="data:image/png;base64,{b64}" width="100" height="100"></body></html>"#
    ));

    // Center of the 100x100 red image should be solidly red (vello applies an
    // sRGB round-trip, so expect a near-pure red rather than an exact value).
    let px = pixel_at(&img, 50, 50);
    assert!(px[0] > 200 && px[1] < 30 && px[2] < 30, "pixel was {px:?}");
}

#[test]
fn renders_a_pre_rasterized_star() {
    let star = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#f5f5f4" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z"/></svg>"##;
    let uri = svg_data_uri(star, 96, 96).unwrap();

    let img = render(&format!(
        r#"<html><body style="background: #111111"><img src="{uri}" width="48" height="48"></body></html>"#
    ));

    // The star stroke is drawn somewhere in the 48x48 area: sample several
    // points and expect at least one near-white pixel (the stroke).
    let mut found_stroke = false;
    for x in (0..48).step_by(3) {
        for y in (0..48).step_by(3) {
            let px = pixel_at(&img, x, y);
            if px[0] > 200 && px[1] > 200 && px[2] > 200 {
                found_stroke = true;
            }
        }
    }
    assert!(found_stroke, "no star stroke pixels found");
}

#[test]
fn rasterizes_svg_to_png() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect x="0" y="0" width="24" height="24" fill="red"/></svg>"#;
    let png = rasterize_svg(svg, 48, 48).unwrap();

    assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
}

#[test]
fn svg_data_uri_has_expected_prefix() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect x="0" y="0" width="24" height="24" fill="red"/></svg>"#;
    let uri = svg_data_uri(svg, 24, 24).unwrap();
    assert!(uri.starts_with("data:image/png;base64,"));
}

#[test]
fn encodes_png_from_buffer() {
    let img = render(r#"<html><body style="background: #ffffff"></body></html>"#);
    let png = img.encode_png().unwrap();
    assert_eq!(&png[..4], &[0x89, b'P', b'N', b'G']);
}
