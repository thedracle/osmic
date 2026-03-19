// OpenMapMarketor facade crate: single-dependency access to the full SDK.
//
// ```toml
// [dependencies]
// omm = "0.1"
// ```

pub use omm_core as core;
pub use omm_osm as osm;
pub use omm_geo as geo;
pub use omm_index as index;
pub use omm_app as app;
pub use omm_style as style;
pub use omm_tiles as tiles;
pub use omm_render as render;
pub use omm_serve as serve;
pub use omm_text as text;

/// Common types re-exported for convenience.
pub mod prelude {
    // Core types
    pub use omm_core::{BBox, Color, Geometry, LonLat, OmmError, OmmResult, PackedCoord, TileCoord, Zoom};

    // OSM data model
    pub use omm_osm::{Feature, FeatureKind, PbfProcessor, TagStore, Tags};
    pub use omm_osm::geojson::load_geojson;

    // Spatial index
    pub use omm_index::{DenseNodeLocationStore, FeatureIndex};

    // App framework
    pub use omm_app::{App, Plugin, PluginGroup};

    // Tile generation
    pub use omm_tiles::pipeline::{TileGenerator, TileGeneratorConfig};
    pub use omm_tiles::pmtiles::PmTilesArchive;

    // Rendering
    pub use omm_render::backend::{RenderBackend, RenderConfig};
    pub use omm_render::skia::SkiaBackend;

    // Style
    pub use omm_style::default_style_json;

    // Server
    pub use omm_serve::{TileServer, TileServerConfig, TileServerPlugin};
}

/// Plugin group that includes all headless (non-GPU) plugins.
pub struct HeadlessPlugins;

impl omm_app::PluginGroup for HeadlessPlugins {
    fn build(self) -> omm_app::PluginGroupBuilder {
        omm_app::PluginGroupBuilder::new()
    }
}

/// Plugin group that includes all default plugins for interactive map rendering.
pub struct DefaultPlugins;

impl omm_app::PluginGroup for DefaultPlugins {
    fn build(self) -> omm_app::PluginGroupBuilder {
        omm_app::PluginGroupBuilder::new()
    }
}
