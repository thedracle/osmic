use omm_core::geometry::Geometry;
use omm_osm::feature::FeatureKind;
use omm_osm::tags::{TagStore, Tags};
#[cfg(feature = "native")]
use pmtiles::TileType;

use crate::coord::TileTransform;

/// Tile output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileFormat {
    Mvt,
    #[cfg(feature = "mlt")]
    Mlt,
}

/// A feature reference for tile encoding.
pub trait TileFeature {
    fn id(&self) -> i64;
    fn kind(&self) -> FeatureKind;
    fn geometry(&self) -> &Geometry;
    fn tags(&self) -> &Tags;
}

/// Abstract tile encoder. Implementations produce format-specific tile bytes
/// from clipped features grouped by layer.
pub trait TileEncoder: Send + Sync {
    /// Encode features with geographic (lon/lat) coordinates.
    /// The transform projects them to tile-local space.
    fn encode_clipped(
        &self,
        extent: u32,
        transform: &TileTransform,
        layer_features: &[(&str, Vec<&dyn TileFeature>)],
        tag_store: &TagStore,
    ) -> Option<Vec<u8>>;

    /// Encode features already in projected tile-local coordinates.
    fn encode_projected(
        &self,
        extent: u32,
        layer_features: &[(&str, Vec<&dyn TileFeature>)],
        tag_store: &TagStore,
    ) -> Option<Vec<u8>>;

    /// The tile format this encoder produces.
    fn format(&self) -> TileFormat;

    /// The PMTiles TileType value for this format.
    #[cfg(feature = "native")]
    fn tile_type(&self) -> TileType;
}
