//! Render HTML documents to PNG images using the Blitz engine, with no browser
//! required. Blitz cannot style SVGs, so pre-rendered images (data URIs) should
//! be used for anything that relies on `fill`/`stroke`; see [`svg`].

pub mod render;
pub mod svg;

pub use render::{RenderConfig, RenderedImage, render_html_to_buffer, render_html_to_png};
