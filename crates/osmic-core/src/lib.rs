pub mod bbox;
pub mod clip;
pub mod color;
pub mod coord;
pub mod error;
pub mod geometry;
pub mod tile;

pub use bbox::BBox;
pub use color::Color;
pub use coord::{LonLat, PackedCoord};
pub use error::{OsmicError, OsmicResult};
pub use geometry::Geometry;
pub use tile::{TileCoord, Zoom};

/// Coordinate lookup storage used by PBF decoding pipelines.
pub trait NodeLocationStore: Send + Sync {
    fn set(&self, node_id: i64, lon: f64, lat: f64);
    fn get(&self, node_id: i64) -> Option<LonLat>;
}
