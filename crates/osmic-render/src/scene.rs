use osmic_core::Color;

/// A backend-agnostic scene graph describing what to render.
///
/// The scene graph is built from map features and styles, then passed
/// to a `RenderBackend` for actual rasterization.
#[derive(Debug, Clone)]
pub struct SceneGraph {
    pub background: Color,
    pub layers: Vec<RenderLayer>,
}

impl SceneGraph {
    pub fn new(background: Color) -> Self {
        Self {
            background,
            layers: Vec::new(),
        }
    }

    pub fn add_layer(&mut self, layer: RenderLayer) {
        self.layers.push(layer);
    }
}

/// A z-ordered rendering layer.
#[derive(Debug, Clone)]
pub struct RenderLayer {
    pub z_order: i32,
    pub features: Vec<RenderFeature>,
}

impl RenderLayer {
    pub fn new(z_order: i32) -> Self {
        Self {
            z_order,
            features: Vec::new(),
        }
    }

    pub fn push(&mut self, feature: RenderFeature) {
        self.features.push(feature);
    }
}

/// A renderable feature with geometry and style.
#[derive(Debug, Clone)]
pub enum RenderFeature {
    /// Filled polygon.
    Fill {
        coords: Vec<Vec<[f32; 2]>>,
        color: Color,
    },
    /// Stroked line.
    Stroke {
        coords: Vec<[f32; 2]>,
        color: Color,
        width: f32,
        cap: LineCap,
        join: LineJoin,
    },
    /// Text label.
    Label {
        position: [f32; 2],
        text: String,
        font_size: f32,
        color: Color,
        halo_color: Option<Color>,
        halo_width: f32,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}
