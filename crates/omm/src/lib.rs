// Facade crate: re-exports all workspace crates for convenient single-dependency usage.
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

pub mod prelude {
    pub use omm_core::{BBox, Color, Geometry, LonLat, OmmError, OmmResult, PackedCoord, TileCoord, Zoom};
    pub use omm_osm::{Feature, FeatureKind, PbfProcessor, TagStore, Tags};
    pub use omm_index::{DenseNodeLocationStore, FeatureIndex};
    pub use omm_app::{App, Plugin, PluginGroup};
}
