use serde::{Deserialize, Serialize};

/// A geographic coordinate in longitude/latitude (WGS84).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LonLat {
    pub lon: f64,
    pub lat: f64,
}

impl LonLat {
    pub const fn new(lon: f64, lat: f64) -> Self {
        Self { lon, lat }
    }

    /// Returns true if this coordinate is within valid WGS84 bounds.
    pub fn is_valid(&self) -> bool {
        (-180.0..=180.0).contains(&self.lon) && (-90.0..=90.0).contains(&self.lat)
    }
}

impl From<LonLat> for geo_types::Coord<f64> {
    fn from(ll: LonLat) -> Self {
        geo_types::Coord {
            x: ll.lon,
            y: ll.lat,
        }
    }
}

impl From<geo_types::Coord<f64>> for LonLat {
    fn from(c: geo_types::Coord<f64>) -> Self {
        Self { lon: c.x, lat: c.y }
    }
}

/// A packed coordinate using f32 for compact storage (8 bytes total).
///
/// Precision: ~1.1 meters at the equator. Sufficient for rendering.
/// Used in `DenseNodeLocationStore` for memory-efficient node storage.
///
/// Coordinates are stored with an offset so that all-zero bytes (as produced
/// by sparse mmap pages that were never written) are distinguishable from
/// valid coordinates. This preserves mmap sparsity: only pages containing
/// actual node data consume disk/RAM.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct PackedCoord {
    lon: f32,
    lat: f32,
}

/// Offsets ensure that valid stored values are never all-zero bits,
/// allowing sparse mmap pages (zero-filled by the OS) to be detected as empty.
const LON_OFFSET: f32 = 256.0;
const LAT_OFFSET: f32 = 128.0;

impl PackedCoord {
    pub fn pack(lon: f64, lat: f64) -> Self {
        Self {
            lon: (lon as f32) + LON_OFFSET,
            lat: (lat as f32) + LAT_OFFSET,
        }
    }

    pub fn unpack(self) -> LonLat {
        LonLat {
            lon: (self.lon - LON_OFFSET) as f64,
            lat: (self.lat - LAT_OFFSET) as f64,
        }
    }

    /// Returns true if this slot was never written.
    ///
    /// Sparse mmap pages are zero-filled by the OS. Since valid packed
    /// coordinates always have non-zero stored values (due to the offset),
    /// a zero check reliably detects unoccupied slots.
    pub fn is_empty(self) -> bool {
        self.lon.to_bits() == 0 && self.lat.to_bits() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- LonLat::is_valid ---

    #[test]
    fn lonlat_valid_within_bounds() {
        assert!(LonLat::new(0.0, 0.0).is_valid());
        assert!(LonLat::new(180.0, 90.0).is_valid());
        assert!(LonLat::new(-180.0, -90.0).is_valid());
        assert!(LonLat::new(179.999, 89.999).is_valid());
    }

    #[test]
    fn lonlat_invalid_outside_bounds() {
        assert!(!LonLat::new(180.001, 0.0).is_valid());
        assert!(!LonLat::new(-180.001, 0.0).is_valid());
        assert!(!LonLat::new(0.0, 90.001).is_valid());
        assert!(!LonLat::new(0.0, -90.001).is_valid());
    }

    #[test]
    fn lonlat_boundary_values_are_valid() {
        // Exact boundary values must be accepted (inclusive range).
        assert!(LonLat::new(180.0, 0.0).is_valid());
        assert!(LonLat::new(-180.0, 0.0).is_valid());
        assert!(LonLat::new(0.0, 90.0).is_valid());
        assert!(LonLat::new(0.0, -90.0).is_valid());
    }

    // --- PackedCoord pack/unpack round-trip ---

    #[test]
    fn packed_coord_roundtrip_origin() {
        let p = PackedCoord::pack(0.0, 0.0);
        let ll = p.unpack();
        // f32 precision: the offset encoding means (0.0 + 256.0) stored as f32
        // then subtracted, so the round-trip should be exact for 0.0.
        assert!((ll.lon - 0.0_f64).abs() < 1e-4);
        assert!((ll.lat - 0.0_f64).abs() < 1e-4);
    }

    #[test]
    fn packed_coord_roundtrip_san_francisco() {
        let lon = -122.4194_f64;
        let lat = 37.7749_f64;
        let p = PackedCoord::pack(lon, lat);
        let ll = p.unpack();
        // f32 has ~7 significant decimal digits; within ~0.001 degree at these values.
        assert!(
            (ll.lon - lon).abs() < 0.001,
            "lon diff: {}",
            (ll.lon - lon).abs()
        );
        assert!(
            (ll.lat - lat).abs() < 0.001,
            "lat diff: {}",
            (ll.lat - lat).abs()
        );
    }

    #[test]
    fn packed_coord_at_negative_extreme_roundtrips() {
        let p = PackedCoord::pack(-180.0, -90.0);
        let ll = p.unpack();
        assert!((ll.lon - (-180.0)).abs() < 0.001);
        assert!((ll.lat - (-90.0)).abs() < 0.001);
    }

    #[test]
    fn packed_coord_at_positive_extreme_roundtrips() {
        let p = PackedCoord::pack(180.0, 90.0);
        let ll = p.unpack();
        assert!((ll.lon - 180.0).abs() < 0.001);
        assert!((ll.lat - 90.0).abs() < 0.001);
    }

    // --- PackedCoord::is_empty semantics ---

    #[test]
    fn packed_coord_origin_is_not_empty() {
        // (0.0, 0.0) packed adds the offsets, so the stored bits are non-zero.
        let p = PackedCoord::pack(0.0, 0.0);
        assert!(
            !p.is_empty(),
            "packed (0,0) must NOT be empty due to offset encoding"
        );
    }

    #[test]
    fn uninitialized_zero_bytes_is_empty() {
        // Simulates an mmap slot that was never written (all zero bytes).
        // SAFETY: PackedCoord is repr(C) with two f32 fields; zeroing it is valid.
        let p: PackedCoord = unsafe { std::mem::zeroed() };
        assert!(p.is_empty(), "all-zero bytes must be detected as empty");
    }
}
