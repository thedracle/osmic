use omm_core::error::OmmResult;

use crate::scene::SceneGraph;

/// Configuration for rendering.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub width: u32,
    pub height: u32,
    pub background: omm_core::Color,
    /// Device pixel ratio (1.0 = standard, 2.0 = retina).
    pub pixel_ratio: f32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: 1024,
            height: 1024,
            background: omm_core::Color::from_hex("#f8f4f0").unwrap(),
            pixel_ratio: 1.0,
        }
    }
}

/// Abstraction over rendering backends (software, GPU).
pub trait RenderBackend {
    /// Initialize the backend with the given configuration.
    fn init(config: &RenderConfig) -> OmmResult<Self>
    where
        Self: Sized;

    /// Render a scene graph to the internal buffer.
    fn render(&mut self, scene: &SceneGraph) -> OmmResult<()>;

    /// Read the rendered pixels as RGBA bytes.
    fn read_pixels(&self) -> Option<Vec<u8>>;

    /// Resize the render target.
    fn resize(&mut self, width: u32, height: u32);
}
