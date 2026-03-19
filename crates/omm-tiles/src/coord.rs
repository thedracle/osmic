use std::f64::consts::PI;

use omm_core::tile::TileCoord;

/// Transforms geographic coordinates (lon/lat) to tile-local MVT coordinates.
///
/// MVT uses a tile-local coordinate system where (0, 0) is the top-left corner
/// and (extent, extent) is the bottom-right. Standard extent is 4096.
///
/// The transform uses Web Mercator projection to correctly map geographic
/// coordinates into the tile grid.
pub struct TileTransform {
    n: f64,
    tx: f64,
    ty: f64,
    extent: f64,
}

impl TileTransform {
    pub fn new(tile: &TileCoord, extent: u32) -> Self {
        Self {
            n: (1u64 << tile.z.0) as f64,
            tx: tile.x as f64,
            ty: tile.y as f64,
            extent: extent as f64,
        }
    }

    /// Convert lon/lat (WGS84 degrees) to tile-local coordinates [0, extent].
    ///
    /// Uses Web Mercator projection for correct mapping in MapLibre/Mapbox clients.
    /// Latitude is clamped to the Mercator limit (~85.051) to avoid infinity/NaN.
    pub fn lon_lat_to_tile(&self, lon: f64, lat: f64) -> (f64, f64) {
        let lat_clamped = lat.clamp(-85.051_129, 85.051_129);
        let lat_rad = lat_clamped.to_radians();

        // Mercator normalized coordinates [0, 1]
        let mx = (lon + 180.0) / 360.0;
        let my = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI) / 2.0;

        // Tile-local coordinates
        let x = (mx * self.n - self.tx) * self.extent;
        let y = (my * self.n - self.ty) * self.extent;

        (x, y)
    }

    pub fn extent(&self) -> u32 {
        self.extent as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omm_core::tile::Zoom;

    #[test]
    fn test_tile_center_maps_to_midpoint() {
        let coord = TileCoord::new(0, 0, Zoom::new(0));
        let transform = TileTransform::new(&coord, 4096);
        let (x, y) = transform.lon_lat_to_tile(0.0, 0.0);
        // (0, 0) should map to center of the z0 tile
        assert!((x - 2048.0).abs() < 1.0);
        assert!((y - 2048.0).abs() < 1.0);
    }

    #[test]
    fn test_tile_corners() {
        let coord = TileCoord::new(0, 0, Zoom::new(1));
        let transform = TileTransform::new(&coord, 4096);
        // Top-left corner of tile 0/0/1 at the Mercator limit
        let (x, y) = transform.lon_lat_to_tile(-180.0, 85.051129);
        assert!(x.abs() < 10.0, "x={x}");
        assert!(y.abs() < 10.0, "y={y}");
    }
}
