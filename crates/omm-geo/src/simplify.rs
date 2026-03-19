use geo::Simplify;
use geo_types::{LineString, MultiPolygon, Polygon};

/// Simplify a line string using the Ramer-Douglas-Peucker algorithm.
pub fn simplify_line(line: &LineString<f64>, tolerance: f64) -> LineString<f64> {
    line.simplify(tolerance)
}

/// Simplify a polygon (exterior and holes) using RDP.
pub fn simplify_polygon(poly: &Polygon<f64>, tolerance: f64) -> Polygon<f64> {
    poly.simplify(tolerance)
}

/// Simplify a multi-polygon.
pub fn simplify_multi_polygon(mp: &MultiPolygon<f64>, tolerance: f64) -> MultiPolygon<f64> {
    mp.simplify(tolerance)
}

/// Compute an appropriate simplification tolerance for the given zoom level.
///
/// At zoom 0, a tile covers ~360 degrees; at zoom 14, ~0.022 degrees.
/// We target sub-pixel precision at MVT extent of 4096.
pub fn tolerance_for_zoom(zoom: u8) -> f64 {
    let tile_degrees = 360.0 / (1u64 << zoom) as f64;
    tile_degrees / 4096.0
}

/// Simplify geometry at the given zoom level.
pub fn simplify_geometry(geom: &omm_core::Geometry, zoom: u8) -> omm_core::Geometry {
    let tol = tolerance_for_zoom(zoom);
    match geom {
        omm_core::Geometry::Point(p) => omm_core::Geometry::Point(*p),
        omm_core::Geometry::Line(ls) => omm_core::Geometry::Line(simplify_line(ls, tol)),
        omm_core::Geometry::Polygon(poly) => {
            omm_core::Geometry::Polygon(simplify_polygon(poly, tol))
        }
        omm_core::Geometry::MultiPolygon(mp) => {
            omm_core::Geometry::MultiPolygon(simplify_multi_polygon(mp, tol))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tolerance_decreases_with_zoom() {
        let t0 = tolerance_for_zoom(0);
        let t14 = tolerance_for_zoom(14);
        assert!(t0 > t14);
        assert!(t14 > 0.0);
    }
}
