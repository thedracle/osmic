//! CompactMonoEncoder: projects geographic coordinates to pixel space and
//! encodes them with variable bit-width delta encoding (TMAP v3).
//!
//! Inspired by Garmin's RGN bitstream format: each polyline's deltas are
//! analyzed to find the minimum bits needed, then packed LSB-first.

use geo_types::{Coord, Polygon};
use osmic_core::{BBox, Geometry};
use osmic_osm::feature::FeatureKind;

use crate::filter::{is_poi, to_category, to_poi_type, to_subcategory};
use crate::format::{encode_type_byte, FeatureCategory, MAX_POI_NAME_LEN};

/// Projects lon/lat coordinates to pixel space within a bounding box.
pub struct PixelProjector {
    min_lon: f64,
    min_lat: f64,
    scale_x: f64,
    scale_y: f64,
    width: u8,
    height: u8,
}

impl PixelProjector {
    pub fn new(bbox: &BBox, width: u8, height: u8) -> Self {
        let lon_range = bbox.max_lon - bbox.min_lon;
        let lat_range = bbox.max_lat - bbox.min_lat;
        Self {
            min_lon: bbox.min_lon,
            min_lat: bbox.min_lat,
            scale_x: if lon_range > 0.0 {
                (width as f64 - 1.0) / lon_range
            } else {
                0.0
            },
            scale_y: if lat_range > 0.0 {
                (height as f64 - 1.0) / lat_range
            } else {
                0.0
            },
            width,
            height,
        }
    }

    /// Project a lon/lat coordinate to pixel space. Y is inverted (north = top = 0).
    pub fn project(&self, lon: f64, lat: f64) -> (u8, u8) {
        let px = ((lon - self.min_lon) * self.scale_x).round() as i16;
        let py = ((lat - self.min_lat) * self.scale_y).round() as i16;
        let py = (self.height as i16 - 1) - py;
        let px = px.clamp(0, self.width as i16 - 1) as u8;
        let py = py.clamp(0, self.height as i16 - 1) as u8;
        (px, py)
    }
}

// ---- Bit writer for LSB-first packing ----

struct BitWriter {
    bytes: Vec<u8>,
    current: u8,
    bit_pos: u8, // 0-7, next bit to write
}

impl BitWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            current: 0,
            bit_pos: 0,
        }
    }

    fn write_bits(&mut self, value: u32, num_bits: u8) {
        let mut v = value;
        let mut remaining = num_bits;
        while remaining > 0 {
            let space = 8 - self.bit_pos;
            let to_write = remaining.min(space);
            let mask = (1u32 << to_write) - 1;
            self.current |= ((v & mask) as u8) << self.bit_pos;
            v >>= to_write;
            self.bit_pos += to_write;
            remaining -= to_write;
            if self.bit_pos >= 8 {
                self.bytes.push(self.current);
                self.current = 0;
                self.bit_pos = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bit_pos > 0 {
            self.bytes.push(self.current);
        }
        self.bytes
    }
}

// ---- Variable bit-width delta encoding ----

/// Compute the minimum number of bits needed to represent a value.
fn bits_needed(max_abs: i16) -> u8 {
    if max_abs == 0 {
        return 1;
    }
    let mut bits = 0u8;
    let mut v = max_abs as u16;
    while v > 0 {
        bits += 1;
        v >>= 1;
    }
    bits.max(1).min(8)
}

struct BitstreamEncoded {
    first_x: u8,
    first_y: u8,
    /// Packed: x_base(2:0) | y_base(5:3) | x_same_sign(6) | y_same_sign(7)
    info_byte: u8,
    bitstream: Vec<u8>,
    coord_count: u8,
}

fn bitstream_encode_points(points: &[(u8, u8)]) -> Option<BitstreamEncoded> {
    if points.len() < 2 {
        return None;
    }

    // Compute deltas
    let mut deltas: Vec<(i16, i16)> = Vec::with_capacity(points.len() - 1);
    let mut prev_x = points[0].0 as i16;
    let mut prev_y = points[0].1 as i16;

    for &(x, y) in &points[1..] {
        let dx = x as i16 - prev_x;
        let dy = y as i16 - prev_y;
        deltas.push((dx, dy));
        prev_x = x as i16;
        prev_y = y as i16;
    }

    if deltas.len() > 254 {
        deltas.truncate(254);
    }

    // Analyze deltas: find max absolute value and sign patterns
    let mut max_abs_x: i16 = 0;
    let mut max_abs_y: i16 = 0;
    let mut x_all_pos = true;
    let mut x_all_neg = true;
    let mut y_all_pos = true;
    let mut y_all_neg = true;
    let mut x_has_zero = false;
    let mut y_has_zero = false;

    for &(dx, dy) in &deltas {
        let ax = dx.abs();
        let ay = dy.abs();
        if ax > max_abs_x { max_abs_x = ax; }
        if ay > max_abs_y { max_abs_y = ay; }
        if dx > 0 { x_all_neg = false; }
        if dx < 0 { x_all_pos = false; }
        if dx == 0 { x_has_zero = true; }
        if dy > 0 { y_all_neg = false; }
        if dy < 0 { y_all_pos = false; }
        if dy == 0 { y_has_zero = true; }
    }

    // Same-sign optimization: if all deltas share a sign (treating 0 as either),
    // we can omit the sign bit from each delta
    let x_same_sign = (x_all_pos || x_all_neg) || (x_has_zero && (x_all_pos || x_all_neg));
    let y_same_sign = (y_all_pos || y_all_neg) || (y_has_zero && (y_all_pos || y_all_neg));
    let x_sign_neg = x_all_neg && !x_all_pos;
    let y_sign_neg = y_all_neg && !y_all_pos;

    // Compute bits needed for magnitude
    let x_mag_bits = bits_needed(max_abs_x);
    let y_mag_bits = bits_needed(max_abs_y);

    // If not same-sign, need an extra bit for sign per delta
    let x_total_bits = if x_same_sign { x_mag_bits } else { x_mag_bits + 1 };
    let y_total_bits = if y_same_sign { y_mag_bits } else { y_mag_bits + 1 };

    // Pack into single info byte: x_base(2:0) | y_base(5:3) | x_same(6) | y_same(7)
    let x_base = (x_total_bits - 1).min(7);
    let y_base = (y_total_bits - 1).min(7);

    let info_byte = (x_base & 0x07)
        | ((y_base & 0x07) << 3)
        | (if x_same_sign { 0x40 } else { 0 })
        | (if y_same_sign { 0x80 } else { 0 });

    // Pack deltas into bitstream
    let mut writer = BitWriter::new();
    let x_bits = x_base + 1;
    let y_bits = y_base + 1;

    // If same_sign, encode the sign direction as the first bit of the bitstream
    if x_same_sign {
        writer.write_bits(if x_sign_neg { 1 } else { 0 }, 1);
    }
    if y_same_sign {
        writer.write_bits(if y_sign_neg { 1 } else { 0 }, 1);
    }

    for &(dx, dy) in &deltas {
        if x_same_sign {
            writer.write_bits(dx.unsigned_abs() as u32, x_bits);
        } else {
            let mag = dx.unsigned_abs() as u32;
            let sign_bit = if dx < 0 { 1u32 } else { 0 };
            writer.write_bits(mag | (sign_bit << x_mag_bits), x_bits);
        }

        if y_same_sign {
            writer.write_bits(dy.unsigned_abs() as u32, y_bits);
        } else {
            let mag = dy.unsigned_abs() as u32;
            let sign_bit = if dy < 0 { 1u32 } else { 0 };
            writer.write_bits(mag | (sign_bit << y_mag_bits), y_bits);
        }
    }

    Some(BitstreamEncoded {
        first_x: points[0].0,
        first_y: points[0].1,
        info_byte,
        bitstream: writer.finish(),
        coord_count: (deltas.len() + 1) as u8,
    })
}

fn write_feature(type_byte: u8, encoded: &BitstreamEncoded, out: &mut Vec<u8>) {
    out.push(type_byte);
    out.push(encoded.coord_count);
    out.push(encoded.first_x);
    out.push(encoded.first_y);
    out.push(encoded.info_byte);
    out.extend_from_slice(&encoded.bitstream);
}

// ---- Public encoding API ----

pub fn encode_line_feature(
    kind: &FeatureKind,
    coords: &[Coord<f64>],
    projector: &PixelProjector,
    out: &mut Vec<u8>,
) -> bool {
    let projected: Vec<(u8, u8)> = coords.iter().map(|c| projector.project(c.x, c.y)).collect();
    let encoded = match bitstream_encode_points(&projected) {
        Some(e) => e,
        None => return false,
    };

    let is_polygon = false;
    let category = match to_category(kind) {
        FeatureCategory::WaterArea => FeatureCategory::WaterLine,
        c => c,
    };
    let subcategory = to_subcategory(kind);
    let type_byte = encode_type_byte(is_polygon, category, subcategory);

    write_feature(type_byte, &encoded, out);
    true
}

pub fn encode_polygon_feature(
    kind: &FeatureKind,
    polygon: &Polygon<f64>,
    projector: &PixelProjector,
    out: &mut Vec<u8>,
) -> bool {
    let coords: Vec<(u8, u8)> = polygon
        .exterior()
        .coords()
        .map(|c| projector.project(c.x, c.y))
        .collect();
    let encoded = match bitstream_encode_points(&coords) {
        Some(e) => e,
        None => return false,
    };

    let is_polygon = true;
    let category = to_category(kind);
    let subcategory = to_subcategory(kind);
    let type_byte = encode_type_byte(is_polygon, category, subcategory);

    write_feature(type_byte, &encoded, out);
    true
}

pub fn encode_poi(
    kind: &FeatureKind,
    lon: f64,
    lat: f64,
    elevation: u16,
    name: &str,
    projector: &PixelProjector,
    out: &mut Vec<u8>,
) {
    let poi_type = to_poi_type(kind);
    let (x, y) = projector.project(lon, lat);
    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len().min(MAX_POI_NAME_LEN);

    out.push(poi_type as u8);
    out.push(x);
    out.push(y);
    out.extend_from_slice(&elevation.to_be_bytes());
    out.push(name_len as u8);
    out.extend_from_slice(&name_bytes[..name_len]);
}

pub fn encode_geometry(
    kind: &FeatureKind,
    geometry: &Geometry,
    projector: &PixelProjector,
    out: &mut Vec<u8>,
) -> u32 {
    if is_poi(kind) {
        return 0;
    }

    match geometry {
        Geometry::Line(line) => {
            let coords: Vec<Coord<f64>> = line.coords().cloned().collect();
            if encode_line_feature(kind, &coords, projector, out) {
                1
            } else {
                0
            }
        }
        Geometry::Polygon(poly) => {
            if kind.is_area() {
                if encode_polygon_feature(kind, poly, projector, out) {
                    1
                } else {
                    0
                }
            } else {
                let coords: Vec<Coord<f64>> = poly.exterior().coords().cloned().collect();
                if encode_line_feature(kind, &coords, projector, out) {
                    1
                } else {
                    0
                }
            }
        }
        Geometry::MultiPolygon(mp) => {
            let mut count = 0;
            for poly in mp.iter() {
                if encode_polygon_feature(kind, poly, projector, out) {
                    count += 1;
                }
            }
            count
        }
        Geometry::Point(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::PoiType;
    use osmic_core::BBox;

    fn test_bbox() -> BBox {
        BBox {
            min_lon: -111.80,
            min_lat: 40.60,
            max_lon: -111.75,
            max_lat: 40.65,
        }
    }

    #[test]
    fn projector_corners() {
        let proj = PixelProjector::new(&test_bbox(), 176, 176);
        let (x, y) = proj.project(-111.80, 40.65);
        assert_eq!((x, y), (0, 0));
        let (x, y) = proj.project(-111.75, 40.60);
        assert_eq!((x, y), (175, 175));
    }

    #[test]
    fn bitstream_encode_simple() {
        let points = vec![(10u8, 20u8), (12, 22), (14, 24)];
        let encoded = bitstream_encode_points(&points).unwrap();
        assert_eq!(encoded.first_x, 10);
        assert_eq!(encoded.first_y, 20);
        assert_eq!(encoded.coord_count, 3);
        // Deltas are (2,2),(2,2) — both positive, max=2, needs 2 bits
        assert_eq!(encoded.info_byte & 0x07, 1); // x_base=1 → 2 bits
        assert_eq!((encoded.info_byte >> 3) & 0x07, 1); // y_base=1 → 2 bits
        assert!(encoded.info_byte & 0x40 != 0); // x_same_sign
        assert!(encoded.info_byte & 0x80 != 0); // y_same_sign
    }

    #[test]
    fn bitstream_encode_mixed_signs() {
        let points = vec![(10u8, 20u8), (12, 18), (10, 20)];
        let encoded = bitstream_encode_points(&points).unwrap();
        // Deltas: (2,-2), (-2, 2) — mixed signs on both axes
        assert_eq!(encoded.info_byte & 0x40, 0); // x NOT same sign
        assert_eq!(encoded.info_byte & 0x80, 0); // y NOT same sign
    }

    #[test]
    fn bitstream_roundtrip() {
        let points = vec![(50u8, 100u8), (53, 98), (55, 95), (52, 97)];
        let encoded = bitstream_encode_points(&points).unwrap();

        let x_bits = (encoded.info_byte & 0x07) + 1;
        let y_bits = ((encoded.info_byte >> 3) & 0x07) + 1;
        let x_same = (encoded.info_byte & 0x40) != 0;
        let y_same = (encoded.info_byte & 0x80) != 0;
        let x_mag_bits = if x_same { x_bits } else { x_bits - 1 };
        let y_mag_bits = if y_same { y_bits } else { y_bits - 1 };

        let bs = &encoded.bitstream;
        let mut bit_off: usize = 0;
        let read_bits = |bs: &[u8], off: &mut usize, n: u8| -> u32 {
            let mut val = 0u32;
            for i in 0..n {
                let byte_idx = *off / 8;
                let bit_idx = *off % 8;
                if byte_idx < bs.len() {
                    val |= (((bs[byte_idx] >> bit_idx) & 1) as u32) << i;
                }
                *off += 1;
            }
            val
        };

        // Read sign direction bits from start of bitstream
        let x_neg = if x_same { read_bits(bs, &mut bit_off, 1) != 0 } else { false };
        let y_neg = if y_same { read_bits(bs, &mut bit_off, 1) != 0 } else { false };

        let mut cur_x = encoded.first_x as i16;
        let mut cur_y = encoded.first_y as i16;
        let mut decoded = vec![(cur_x as u8, cur_y as u8)];

        for _ in 0..3 {
            let xv = read_bits(bs, &mut bit_off, x_bits);
            let dx = if x_same {
                let mag = xv as i16;
                if x_neg { -mag } else { mag }
            } else {
                let mag = (xv & ((1 << x_mag_bits) - 1)) as i16;
                let sign = (xv >> x_mag_bits) & 1;
                if sign != 0 { -mag } else { mag }
            };

            let yv = read_bits(bs, &mut bit_off, y_bits);
            let dy = if y_same {
                let mag = yv as i16;
                if y_neg { -mag } else { mag }
            } else {
                let mag = (yv & ((1 << y_mag_bits) - 1)) as i16;
                let sign = (yv >> y_mag_bits) & 1;
                if sign != 0 { -mag } else { mag }
            };

            cur_x += dx;
            cur_y += dy;
            decoded.push((cur_x as u8, cur_y as u8));
        }

        assert_eq!(decoded, points);
    }

    #[test]
    fn bitstream_encode_handles_empty() {
        let points = vec![(10u8, 20u8)];
        assert!(bitstream_encode_points(&points).is_none());
    }

    #[test]
    fn poi_encoding() {
        let kind = FeatureKind::Natural(osmic_osm::feature::NaturalKind::Peak);
        let proj = PixelProjector::new(&test_bbox(), 176, 176);
        let mut buf = Vec::new();
        encode_poi(&kind, -111.775, 40.625, 3200, "Mt Test", &proj, &mut buf);
        assert_eq!(buf[0], PoiType::Peak as u8);
        assert_eq!(buf[5], 7);
        assert_eq!(&buf[6..13], b"Mt Test");
    }
}
