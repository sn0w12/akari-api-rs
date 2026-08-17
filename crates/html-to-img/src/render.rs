//! Render HTML documents to raster images (PNG) using the Blitz engine.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyrender::render_to_buffer;
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::{DocumentConfig, net::Resource};
use blitz_html::HtmlDocument;
use blitz_net::{MpscCallback, Provider};
use blitz_paint::paint_scene;
use blitz_traits::shell::{ColorScheme, Viewport};
use thiserror::Error;

/// Configuration for rendering HTML to an image.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Output width in logical pixels.
    pub width: u32,
    /// Output height in logical pixels.
    pub height: u32,
    /// Device pixel ratio applied to the output buffer.
    pub scale: f32,
    /// How long to wait for network resources (images, stylesheets) to load
    /// before rendering with whatever has arrived.
    pub timeout: Duration,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: 1200,
            height: 630,
            scale: 1.0,
            timeout: Duration::from_secs(15),
        }
    }
}

/// A rendered image in raw RGBA8 format.
#[derive(Debug, Clone)]
pub struct RenderedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl RenderedImage {
    /// Encodes the image as PNG bytes.
    pub fn encode_png(&self) -> Result<Vec<u8>, Error> {
        encode_png(&self.rgba, self.width, self.height)
    }
}

/// Errors that can occur while rendering HTML or SVG.
#[derive(Debug, Error)]
pub enum Error {
    #[error("SVG parse failed: {0}")]
    SvgParse(String),
    #[error("SVG rasterization failed: {0}")]
    SvgRaster(String),
    #[error("PNG encoding failed: {0}")]
    PngEncode(String),
}

/// Renders HTML to a raw RGBA8 buffer.
///
/// # Panics
///
/// Panics if called outside of a Tokio runtime context, because the network
/// provider used to fetch images requires a runtime handle. Call it from an
/// async context or via [`tokio::task::spawn_blocking`].
pub fn render_html_to_buffer(html: &str, config: &RenderConfig) -> Result<RenderedImage, Error> {
    let viewport = Viewport::new(
        config.width,
        config.height,
        config.scale,
        ColorScheme::Light,
    );

    let (mut rx, callback) = MpscCallback::<Resource>::new();
    let provider = Arc::new(Provider::new(Arc::new(callback)));

    let doc_config = DocumentConfig {
        viewport: Some(viewport),
        net_provider: Some(Arc::clone(&provider) as _),
        ..Default::default()
    };

    let mut document = HtmlDocument::from_html(html, doc_config);

    // Resolve layout, drain loaded resources back into the document, and keep
    // resolving until the network provider reports no in-flight requests.
    let deadline = Instant::now() + config.timeout;
    loop {
        document.resolve(0.0);

        while let Ok((_doc_id, resource)) = rx.try_recv() {
            document.load_resource(resource);
        }

        if provider.is_empty() {
            break;
        }
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    // Final resolve so resources drained on the last iteration are laid out.
    document.resolve(0.0);

    let render_width = (config.width as f64 * f64::from(config.scale)) as u32;
    let render_height = (config.height as f64 * f64::from(config.scale)) as u32;

    let buffer = render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| {
            paint_scene(
                scene,
                document.as_ref(),
                f64::from(config.scale),
                render_width,
                render_height,
            );
        },
        render_width,
        render_height,
    );

    Ok(RenderedImage {
        width: render_width,
        height: render_height,
        rgba: buffer,
    })
}

/// Renders HTML and returns the result as PNG bytes.
///
/// # Panics
///
/// Panics if called outside of a Tokio runtime context. See
/// [`render_html_to_buffer`].
pub fn render_html_to_png(html: &str, config: &RenderConfig) -> Result<Vec<u8>, Error> {
    render_html_to_buffer(html, config)?.encode_png()
}

/// Encodes an RGBA8 buffer to PNG bytes.
fn encode_png(buffer: &[u8], width: u32, height: u32) -> Result<Vec<u8>, Error> {
    let mut output = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);

        let mut writer = encoder
            .write_header()
            .map_err(|e| Error::PngEncode(e.to_string()))?;
        writer
            .write_image_data(buffer)
            .map_err(|e| Error::PngEncode(e.to_string()))?;
    }
    Ok(output)
}
