//! Binary format definition for the CompactTrailBlob.
//!
//! Target: Garmin Connect IQ devices with monochrome displays (e.g., Instinct 2 Solar 176x176).
//! Design goal: 10-20 KB per trail area, parseable by Monkey C with minimal memory.

/// Magic bytes identifying a CompactTrailBlob.
pub const MAGIC: [u8; 4] = *b"TMAP";

/// Current format version (3 = variable bit-width delta encoding).
pub const VERSION: u8 = 3;

/// Size of the fixed header in bytes.
pub const HEADER_SIZE: usize = 32;

/// Maximum name length for POIs and labels (bytes, UTF-8).
pub const MAX_POI_NAME_LEN: usize = 32;

/// Maximum name length for road/trail labels.
pub const MAX_LABEL_NAME_LEN: usize = 24;

/// Feature categories encoded in the type byte (bits 6-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FeatureCategory {
    Trail = 0,
    Road = 1,
    WaterLine = 2,
    WaterArea = 3,
    ContourMinor = 4,
    ContourMajor = 5,
    NaturalArea = 6,
    Boundary = 7,
}

/// POI types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PoiType {
    Peak = 0,
    CampSite = 1,
    Viewpoint = 2,
    Trailhead = 3,
    WaterSource = 4,
    PicnicSite = 5,
    Parking = 6,
    Village = 7,
    Town = 8,
    City = 9,
    Other = 15,
}

/// Fixed header for the compact trail blob.
#[derive(Debug, Clone)]
pub struct CompactHeader {
    pub version: u8,
    pub display_width: u8,
    pub display_height: u8,
    pub contour_interval: u8,
    pub bbox_min_lon: i32,
    pub bbox_min_lat: i32,
    pub bbox_max_lon: i32,
    pub bbox_max_lat: i32,
    pub feature_count: u16,
    pub poi_count: u16,
    pub label_count: u16,
}

impl CompactHeader {
    /// Serialize header to bytes (32 bytes).
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0..4].copy_from_slice(&MAGIC);
        buf[4] = self.version;
        buf[5] = self.display_width;
        buf[6] = self.display_height;
        buf[7] = self.contour_interval;
        buf[8..12].copy_from_slice(&self.bbox_min_lon.to_be_bytes());
        buf[12..16].copy_from_slice(&self.bbox_min_lat.to_be_bytes());
        buf[16..20].copy_from_slice(&self.bbox_max_lon.to_be_bytes());
        buf[20..24].copy_from_slice(&self.bbox_max_lat.to_be_bytes());
        buf[24..26].copy_from_slice(&self.feature_count.to_be_bytes());
        buf[26..28].copy_from_slice(&self.poi_count.to_be_bytes());
        buf[28..30].copy_from_slice(&self.label_count.to_be_bytes());
        // bytes 30..32 reserved
        buf
    }

    /// Deserialize header from bytes.
    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() < HEADER_SIZE || buf[0..4] != MAGIC {
            return None;
        }
        Some(Self {
            version: buf[4],
            display_width: buf[5],
            display_height: buf[6],
            contour_interval: buf[7],
            bbox_min_lon: i32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
            bbox_min_lat: i32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]),
            bbox_max_lon: i32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]),
            bbox_max_lat: i32::from_be_bytes([buf[20], buf[21], buf[22], buf[23]]),
            feature_count: u16::from_be_bytes([buf[24], buf[25]]),
            poi_count: u16::from_be_bytes([buf[26], buf[27]]),
            label_count: u16::from_be_bytes([buf[28], buf[29]]),
        })
    }
}

/// Encode a type byte from category and subcategory.
/// Bit 7: 0=line, 1=polygon. Bits 6-4: category. Bits 3-0: subcategory.
pub fn encode_type_byte(is_polygon: bool, category: FeatureCategory, subcategory: u8) -> u8 {
    let poly_bit = if is_polygon { 0x80 } else { 0 };
    poly_bit | ((category as u8 & 0x07) << 4) | (subcategory & 0x0F)
}

/// Decode a type byte into (is_polygon, category_bits, subcategory).
pub fn decode_type_byte(byte: u8) -> (bool, u8, u8) {
    let is_polygon = byte & 0x80 != 0;
    let category = (byte >> 4) & 0x07;
    let subcategory = byte & 0x0F;
    (is_polygon, category, subcategory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let header = CompactHeader {
            version: VERSION,
            display_width: 176,
            display_height: 176,
            contour_interval: 20,
            bbox_min_lon: -111_800_000,
            bbox_min_lat: 40_600_000,
            bbox_max_lon: -111_750_000,
            bbox_max_lat: 40_650_000,
            feature_count: 150,
            poi_count: 12,
            label_count: 5,
        };
        let bytes = header.to_bytes();
        let decoded = CompactHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.version, VERSION);
        assert_eq!(decoded.display_width, 176);
        assert_eq!(decoded.display_height, 176);
        assert_eq!(decoded.contour_interval, 20);
        assert_eq!(decoded.bbox_min_lon, -111_800_000);
        assert_eq!(decoded.bbox_min_lat, 40_600_000);
        assert_eq!(decoded.feature_count, 150);
        assert_eq!(decoded.poi_count, 12);
    }

    #[test]
    fn type_byte_roundtrip() {
        let byte = encode_type_byte(false, FeatureCategory::Trail, 3);
        let (is_poly, cat, sub) = decode_type_byte(byte);
        assert!(!is_poly);
        assert_eq!(cat, FeatureCategory::Trail as u8);
        assert_eq!(sub, 3);

        let byte2 = encode_type_byte(true, FeatureCategory::WaterArea, 0);
        let (is_poly2, cat2, sub2) = decode_type_byte(byte2);
        assert!(is_poly2);
        assert_eq!(cat2, FeatureCategory::WaterArea as u8);
        assert_eq!(sub2, 0);
    }

    #[test]
    fn invalid_magic_returns_none() {
        let mut bytes = [0u8; HEADER_SIZE];
        bytes[0..4].copy_from_slice(b"NOPE");
        assert!(CompactHeader::from_bytes(&bytes).is_none());
    }

    #[test]
    fn too_short_returns_none() {
        assert!(CompactHeader::from_bytes(&[0u8; 16]).is_none());
    }
}
