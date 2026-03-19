use std::f64::consts::PI;

use omm_core::bbox::BBox;

/// WGS84 semi-major axis (Earth radius at equator) in meters.
pub const WGS84_A: f64 = 6_378_137.0;

/// Maximum latitude for Web Mercator (degrees).
pub const MAX_LATITUDE: f64 = 85.051_129;

/// Convert WGS84 lon/lat (degrees) to Web Mercator (meters).
pub fn lon_lat_to_web_mercator(lon: f64, lat: f64) -> (f64, f64) {
    let lat_clamped = lat.clamp(-MAX_LATITUDE, MAX_LATITUDE);
    let lon_rad = lon.to_radians();
    let lat_rad = lat_clamped.to_radians();

    let x = WGS84_A * lon_rad;
    let y = WGS84_A * ((PI / 4.0 + lat_rad / 2.0).tan()).ln();

    (x, y)
}

/// Convert Web Mercator (meters) to WGS84 lon/lat (degrees).
pub fn web_mercator_to_lon_lat(x: f64, y: f64) -> (f64, f64) {
    let lon = (x / WGS84_A).to_degrees();
    let lat = (2.0 * (y / WGS84_A).exp().atan() - PI / 2.0).to_degrees();
    (lon, lat)
}

/// Convert lon/lat to slippy map tile coordinates at the given zoom.
pub fn lon_lat_to_tile(lon: f64, lat: f64, zoom: u8) -> (u32, u32) {
    let n = (1u64 << zoom) as f64;
    let lat_rad = lat.to_radians();

    let x = ((lon + 180.0) / 360.0 * n).floor() as u32;
    let y = ((1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI) / 2.0 * n).floor() as u32;

    let max = (1u32 << zoom) - 1;
    (x.min(max), y.min(max))
}

/// Get the WGS84 bounding box for a tile at (x, y, zoom).
pub fn tile_bbox(x: u32, y: u32, zoom: u8) -> BBox {
    let n = (1u64 << zoom) as f64;

    let min_lon = x as f64 / n * 360.0 - 180.0;
    let max_lon = (x + 1) as f64 / n * 360.0 - 180.0;

    let max_lat = (PI * (1.0 - 2.0 * y as f64 / n)).sinh().atan().to_degrees();
    let min_lat = (PI * (1.0 - 2.0 * (y + 1) as f64 / n))
        .sinh()
        .atan()
        .to_degrees();

    BBox::new(min_lon, min_lat, max_lon, max_lat)
}

/// Convert a BBox to the range of tiles that cover it at the given zoom.
pub fn bbox_to_tile_range(bbox: &BBox, zoom: u8) -> (u32, u32, u32, u32) {
    let (min_x, max_y) = lon_lat_to_tile(bbox.min_lon, bbox.min_lat, zoom);
    let (max_x, min_y) = lon_lat_to_tile(bbox.max_lon, bbox.max_lat, zoom);
    (min_x, min_y, max_x, max_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mercator_roundtrip() {
        let (mx, my) = lon_lat_to_web_mercator(-122.4194, 37.7749);
        let (lon, lat) = web_mercator_to_lon_lat(mx, my);
        assert!((lon - (-122.4194)).abs() < 1e-6);
        assert!((lat - 37.7749).abs() < 1e-6);
    }

    #[test]
    fn test_tile_at_origin() {
        let (x, y) = lon_lat_to_tile(0.0, 0.0, 0);
        assert_eq!(x, 0);
        assert_eq!(y, 0);
    }

    #[test]
    fn test_tile_bbox_z0() {
        let bb = tile_bbox(0, 0, 0);
        assert!((bb.min_lon - (-180.0)).abs() < 1e-6);
        assert!((bb.max_lon - 180.0).abs() < 1e-6);
    }
}
