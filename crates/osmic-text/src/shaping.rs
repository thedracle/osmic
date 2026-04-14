use cosmic_text::{
    Attrs, Buffer, Color as CosmicColor, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use tracing::info;

/// Text shaper and glyph rasterizer using cosmic-text.
pub struct TextShaper {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl TextShaper {
    /// Create a new text shaper with system fonts loaded.
    pub fn new() -> Self {
        info!("Loading system fonts...");
        let font_system = FontSystem::new();
        let swash_cache = SwashCache::new();
        info!("Font system ready");
        Self {
            font_system,
            swash_cache,
        }
    }

    /// Measure text dimensions (width, height) for the given string and font size.
    pub fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32) {
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let attrs = Attrs::new().family(Family::SansSerif);
        buffer.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut width: f32 = 0.0;
        let mut height: f32 = 0.0;
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height += run.line_height;
        }
        (width, height)
    }

    /// Rasterize text into an RGBA pixel buffer.
    ///
    /// Returns (width, height, rgba_pixels).
    pub fn rasterize(
        &mut self,
        text: &str,
        font_size: f32,
        color: osmic_core::Color,
    ) -> (u32, u32, Vec<u8>) {
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        let attrs = Attrs::new().family(Family::SansSerif);
        buffer.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.font_system, false);

        // Measure
        let (text_w, text_h) = {
            let mut w: f32 = 0.0;
            let mut h: f32 = 0.0;
            for run in buffer.layout_runs() {
                w = w.max(run.line_w);
                h += run.line_height;
            }
            (w.ceil() as u32 + 2, h.ceil() as u32 + 2)
        };

        if text_w == 0 || text_h == 0 {
            return (0, 0, vec![]);
        }

        let cr = (color.r * 255.0) as u8;
        let cg = (color.g * 255.0) as u8;
        let cb = (color.b * 255.0) as u8;

        let cosmic_color = CosmicColor::rgba(cr, cg, cb, 255);

        // Rasterize glyphs
        let mut pixels = vec![0u8; (text_w * text_h * 4) as usize];
        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            cosmic_color,
            |x, y, _w, _h, color| {
                let px = x as u32;
                let py = y as u32;
                if px < text_w && py < text_h {
                    let idx = ((py * text_w + px) * 4) as usize;
                    if idx + 3 < pixels.len() {
                        let a = color.a();
                        if a > 0 {
                            // Alpha blend
                            let src_a = a as f32 / 255.0;
                            let dst_a = pixels[idx + 3] as f32 / 255.0;
                            let out_a = src_a + dst_a * (1.0 - src_a);
                            if out_a > 0.0 {
                                pixels[idx] = ((color.r() as f32 * src_a
                                    + pixels[idx] as f32 * dst_a * (1.0 - src_a))
                                    / out_a) as u8;
                                pixels[idx + 1] = ((color.g() as f32 * src_a
                                    + pixels[idx + 1] as f32 * dst_a * (1.0 - src_a))
                                    / out_a)
                                    as u8;
                                pixels[idx + 2] = ((color.b() as f32 * src_a
                                    + pixels[idx + 2] as f32 * dst_a * (1.0 - src_a))
                                    / out_a)
                                    as u8;
                                pixels[idx + 3] = (out_a * 255.0) as u8;
                            }
                        }
                    }
                }
            },
        );

        (text_w, text_h, pixels)
    }
}

impl Default for TextShaper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_nonempty_text_returns_positive_dimensions() {
        let mut shaper = TextShaper::new();
        let (w, h) = shaper.measure("hello", 16.0);
        assert!(w > 0.0, "width should be positive for non-empty text");
        assert!(h > 0.0, "height should be positive for non-empty text");
    }

    #[test]
    fn measure_longer_text_is_wider_than_shorter() {
        let mut shaper = TextShaper::new();
        let (w_short, _) = shaper.measure("a", 16.0);
        let (w_long, _) = shaper.measure("aaaaaaaaaa", 16.0);
        assert!(w_long > w_short);
    }

    #[test]
    fn rasterize_produces_rgba_buffer_of_declared_size() {
        let mut shaper = TextShaper::new();
        let (w, h, pixels) =
            shaper.rasterize("hi", 16.0, osmic_core::Color::rgba(1.0, 1.0, 1.0, 1.0));
        assert!(
            w > 0 && h > 0,
            "rasterized text should have positive dimensions"
        );
        assert_eq!(
            pixels.len(),
            (w as usize) * (h as usize) * 4,
            "pixel buffer length must match w * h * RGBA"
        );
    }
}
