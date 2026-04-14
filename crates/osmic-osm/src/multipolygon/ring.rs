//! Proto-rings built incrementally during segment chaining.
//!
//! Each `ProtoRing` maintains a running 2× signed area via the cross-product
//! shoelace form `Σ (x_i * y_{i+1} - x_{i+1} * y_i)`. This has two nice
//! properties:
//!
//! 1. It can be accumulated **online** — we add the contribution of each
//!    segment as we chain it, so orientation is known the instant the ring
//!    closes. No O(n) second pass.
//! 2. Reversing a ring negates the signed area — so after a reversal we
//!    simply flip the sign of `signed_area_2x` without recomputation.
//!
//! Sign convention: **positive = counter-clockwise** in standard math
//! coordinates (y up). This matches `geo::Winding` and GeoJSON RFC 7946.

use geo_types::Coord;

use crate::multipolygon::segment::Role;

pub struct ProtoRing {
    pub coords: Vec<Coord<f64>>,
    pub role: Role,
    /// Running 2× signed area (shoelace). Positive = CCW, negative = CW.
    /// Only meaningful for closed rings.
    pub signed_area_2x: f64,
}

impl ProtoRing {
    pub fn new(role: Role) -> Self {
        Self {
            coords: Vec::new(),
            role,
            signed_area_2x: 0.0,
        }
    }

    /// Append a segment `start → end`. Caller must ensure `start` equals
    /// the current ring's last coordinate, or the ring is empty.
    pub fn append(&mut self, start: Coord<f64>, end: Coord<f64>) {
        // Cross-product shoelace contribution: x_i * y_{i+1} - x_{i+1} * y_i.
        // Positive sum = CCW (counter-clockwise) in standard math coordinates,
        // matching geo::Winding and GeoJSON RFC 7946.
        self.signed_area_2x += start.x * end.y - end.x * start.y;
        if self.coords.is_empty() {
            self.coords.push(start);
        }
        self.coords.push(end);
    }

    /// Whether the ring is closed (≥4 coords and last equals first).
    /// OGC polygon rings require at least 4 points (3 unique + repeat).
    pub fn is_closed(&self) -> bool {
        self.coords.len() >= 4 && self.coords.first() == self.coords.last()
    }

    pub fn is_ccw(&self) -> bool {
        self.signed_area_2x > 0.0
    }

    pub fn is_cw(&self) -> bool {
        self.signed_area_2x < 0.0
    }

    /// Reverse the ring's coordinate order. Area magnitude is unchanged;
    /// sign flips.
    pub fn reverse(&mut self) {
        self.coords.reverse();
        self.signed_area_2x = -self.signed_area_2x;
    }

    /// Return the first coordinate, or `None` if the ring is empty.
    pub fn first(&self) -> Option<Coord<f64>> {
        self.coords.first().copied()
    }

    /// Return the last coordinate, or `None` if the ring is empty.
    pub fn last(&self) -> Option<Coord<f64>> {
        self.coords.last().copied()
    }

    pub fn len(&self) -> usize {
        self.coords.len()
    }

    pub fn is_empty(&self) -> bool {
        self.coords.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: f64, y: f64) -> Coord<f64> {
        Coord { x, y }
    }

    /// A CCW unit square built segment-by-segment should have
    /// signed_area_2x = +2 (actual area 1, shoelace returns 2× area).
    #[test]
    fn ccw_square_running_area() {
        let mut ring = ProtoRing::new(Role::Outer);
        ring.append(c(0.0, 0.0), c(1.0, 0.0));
        ring.append(c(1.0, 0.0), c(1.0, 1.0));
        ring.append(c(1.0, 1.0), c(0.0, 1.0));
        ring.append(c(0.0, 1.0), c(0.0, 0.0));
        assert!(ring.is_closed());
        assert!(ring.is_ccw(), "CCW square should have positive signed area");
        assert!((ring.signed_area_2x - 2.0).abs() < 1e-9);
    }

    /// A CW unit square should have signed_area_2x = -2.
    #[test]
    fn cw_square_running_area() {
        let mut ring = ProtoRing::new(Role::Outer);
        ring.append(c(0.0, 0.0), c(0.0, 1.0));
        ring.append(c(0.0, 1.0), c(1.0, 1.0));
        ring.append(c(1.0, 1.0), c(1.0, 0.0));
        ring.append(c(1.0, 0.0), c(0.0, 0.0));
        assert!(ring.is_closed());
        assert!(ring.is_cw(), "CW square should have negative signed area");
        assert!((ring.signed_area_2x - (-2.0)).abs() < 1e-9);
    }

    /// Reversing flips the sign without changing magnitude.
    #[test]
    fn reverse_flips_sign() {
        let mut ring = ProtoRing::new(Role::Outer);
        ring.append(c(0.0, 0.0), c(1.0, 0.0));
        ring.append(c(1.0, 0.0), c(1.0, 1.0));
        ring.append(c(1.0, 1.0), c(0.0, 1.0));
        ring.append(c(0.0, 1.0), c(0.0, 0.0));
        let area_before = ring.signed_area_2x;
        ring.reverse();
        assert!((ring.signed_area_2x + area_before).abs() < 1e-9);
        // Coord order also reversed
        assert_eq!(ring.coords.first(), Some(&c(0.0, 0.0)));
        assert_eq!(ring.coords.last(), Some(&c(0.0, 0.0)));
    }

    /// Less than 4 coords is not considered a closed ring.
    #[test]
    fn short_ring_is_not_closed() {
        let mut ring = ProtoRing::new(Role::Outer);
        ring.append(c(0.0, 0.0), c(1.0, 0.0));
        ring.append(c(1.0, 0.0), c(0.0, 0.0));
        assert!(!ring.is_closed(), "a 3-coord ring is not valid");
    }
}
