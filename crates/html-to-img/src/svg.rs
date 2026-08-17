//! Rasterize SVG strings into PNG images.
//!
//! Blitz can render SVG shapes but does not support styling them (no `fill`,
//! `stroke`, etc.), so anything that needs SVG styling must be pre-rendered
//! with this module and embedded into the HTML as a `data:` URI.

use base64::Engine;
use thiserror::Error;

/// Errors that can occur while rasterizing SVG.
#[derive(Debug, Error)]
pub enum SvgError {
    #[error("SVG parse failed: {0}")]
    Parse(String),
    #[error("SVG rasterization failed: {0}")]
    Raster(String),
}

impl From<SvgError> for crate::render::Error {
    fn from(value: SvgError) -> Self {
        match value {
            SvgError::Parse(msg) => crate::render::Error::SvgParse(msg),
            SvgError::Raster(msg) => crate::render::Error::SvgRaster(msg),
        }
    }
}

/// Rasterizes an SVG document to PNG bytes, scaling it to the requested size.
pub fn rasterize_svg(svg: &str, width: u32, height: u32) -> Result<Vec<u8>, SvgError> {
    let opt = resvg::usvg::Options::default();
    let tree =
        resvg::usvg::Tree::from_str(svg, &opt).map_err(|e| SvgError::Parse(e.to_string()))?;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| SvgError::Raster("failed to allocate output bitmap".to_string()))?;

    let tree_size = tree.size();
    let scale = if tree_size.width() > 0.0 && tree_size.height() > 0.0 {
        (width as f32 / tree_size.width()).min(height as f32 / tree_size.height())
    } else {
        1.0
    };
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    pixmap
        .encode_png()
        .map_err(|e| SvgError::Raster(e.to_string()))
}

/// Rasterizes an SVG and returns it as a `data:image/png;base64,...` URI
/// suitable for use as an `<img src>`.
pub fn svg_data_uri(svg: &str, width: u32, height: u32) -> Result<String, SvgError> {
    let png = rasterize_svg(svg, width, height)?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png)
    ))
}
