use geo_types::{Coord, LineString, Polygon};

use osmic_core::geometry::Geometry;

/// A decoded MVT feature with layer name, class, name, extra tags, and geometry.
#[derive(Debug)]
pub struct DecodedFeature {
    pub layer: String,
    pub class: Option<String>,
    pub name: Option<String>,
    /// Additional tags (address, phone, website, etc.)
    pub tags: Vec<(String, String)>,
    pub geometry: Geometry,
}

/// Decode an MVT tile's raw protobuf bytes into features.
///
/// Converts tile-local coordinates back to geographic lon/lat using the
/// inverse of the Web Mercator tile projection.
pub fn decode_tile(data: &[u8], _z: u8, x: u32, y: u32, n: f64) -> Vec<DecodedFeature> {
    let mut features = Vec::new();
    let mut pos = 0;

    while pos < data.len() {
        let (tag, new_pos) = read_varint(data, pos);
        let field = tag >> 3;
        let wire = tag & 7;
        pos = new_pos;

        if wire == 2 && field == 3 {
            let (length, new_pos) = read_varint(data, pos);
            pos = new_pos;
            let layer_data = &data[pos..pos + length as usize];
            decode_layer(layer_data, x, y, n, &mut features);
            pos += length as usize;
        } else {
            break;
        }
    }

    features
}

fn decode_layer(
    data: &[u8],
    x: u32,
    y: u32,
    n: f64,
    out: &mut Vec<DecodedFeature>,
) {
    let mut pos = 0;
    let mut name = String::new();
    let mut keys: Vec<String> = Vec::new();
    let mut values: Vec<String> = Vec::new();
    let mut raw_features: Vec<&[u8]> = Vec::new();
    let mut extent: u32 = 4096;

    while pos < data.len() {
        let (tag, new_pos) = read_varint(data, pos);
        let field = tag >> 3;
        let wire = tag & 7;
        pos = new_pos;

        match (field, wire) {
            (1, 2) => {
                // Layer name
                let (len, new_pos) = read_varint(data, pos);
                pos = new_pos;
                name = String::from_utf8_lossy(&data[pos..pos + len as usize]).to_string();
                pos += len as usize;
            }
            (2, 2) => {
                // Feature
                let (len, new_pos) = read_varint(data, pos);
                pos = new_pos;
                raw_features.push(&data[pos..pos + len as usize]);
                pos += len as usize;
            }
            (3, 2) => {
                // Key
                let (len, new_pos) = read_varint(data, pos);
                pos = new_pos;
                keys.push(String::from_utf8_lossy(&data[pos..pos + len as usize]).to_string());
                pos += len as usize;
            }
            (4, 2) => {
                // Value
                let (len, new_pos) = read_varint(data, pos);
                pos = new_pos;
                let val = decode_value(&data[pos..pos + len as usize]);
                values.push(val);
                pos += len as usize;
            }
            (5, 0) => {
                // Extent
                let (val, new_pos) = read_varint(data, pos);
                extent = val as u32;
                pos = new_pos;
            }
            (_, 0) => {
                let (_, new_pos) = read_varint(data, pos);
                pos = new_pos;
            }
            (_, 2) => {
                let (len, new_pos) = read_varint(data, pos);
                pos = new_pos + len as usize;
            }
            (_, 5) => pos += 4,
            (_, 1) => pos += 8,
            _ => break,
        }
    }

    let ext_f = extent as f64;

    for feat_data in raw_features {
        if let Some(feat) = decode_feature(feat_data, &name, &keys, &values, ext_f, n, x, y) {
            out.push(feat);
        }
    }
}

fn decode_feature(
    data: &[u8],
    layer_name: &str,
    keys: &[String],
    values: &[String],
    extent: f64,
    n: f64,
    tx: u32,
    ty: u32,
) -> Option<DecodedFeature> {
    let mut pos = 0;
    let mut geom_type: u32 = 0;
    let mut geom_data: &[u8] = &[];
    let mut tag_indices: Vec<u64> = Vec::new();

    while pos < data.len() {
        let (tag, new_pos) = read_varint(data, pos);
        let field = tag >> 3;
        let wire = tag & 7;
        pos = new_pos;

        match (field, wire) {
            (3, 0) => {
                let (val, new_pos) = read_varint(data, pos);
                geom_type = val as u32;
                pos = new_pos;
            }
            (4, 2) => {
                let (len, new_pos) = read_varint(data, pos);
                pos = new_pos;
                geom_data = &data[pos..pos + len as usize];
                pos += len as usize;
            }
            (2, 2) => {
                // Tags (packed uint32)
                let (len, new_pos) = read_varint(data, pos);
                pos = new_pos;
                let tag_end = pos + len as usize;
                while pos < tag_end {
                    let (v, new_pos) = read_varint(data, pos);
                    tag_indices.push(v);
                    pos = new_pos;
                }
            }
            (_, 0) => {
                let (_, new_pos) = read_varint(data, pos);
                pos = new_pos;
            }
            (_, 2) => {
                let (len, new_pos) = read_varint(data, pos);
                pos = new_pos + len as usize;
            }
            (_, 5) => pos += 4,
            (_, 1) => pos += 8,
            _ => break,
        }
    }

    // Extract class, name, and extra tags
    let mut class = None;
    let mut feat_name = None;
    let mut extra_tags = Vec::new();
    let mut i = 0;
    while i + 1 < tag_indices.len() {
        let ki = tag_indices[i] as usize;
        let vi = tag_indices[i + 1] as usize;
        if ki < keys.len() && vi < values.len() {
            match keys[ki].as_str() {
                "class" => class = Some(values[vi].clone()),
                "name" => feat_name = Some(values[vi].clone()),
                _ => extra_tags.push((keys[ki].clone(), values[vi].clone())),
            }
        }
        i += 2;
    }

    // Decode geometry
    let coords = decode_geometry_commands(geom_data);
    if coords.is_empty() {
        return None;
    }

    // Convert tile-local to lon/lat
    let to_lonlat = |tx_local: f64, ty_local: f64| -> Coord<f64> {
        let mx = (tx as f64 + tx_local / extent) / n;
        let my = (ty as f64 + ty_local / extent) / n;
        let lon = mx * 360.0 - 180.0;
        let lat = (std::f64::consts::PI * (1.0 - 2.0 * my)).sinh().atan().to_degrees();
        Coord { x: lon, y: lat }
    };

    let geometry = match geom_type {
        1 => {
            // Point
            if let Some(&(cx, cy)) = coords.first().and_then(|ring| ring.first()) {
                let c = to_lonlat(cx as f64, cy as f64);
                Geometry::Point(geo_types::Point(c))
            } else {
                return None;
            }
        }
        2 => {
            // LineString
            if let Some(ring) = coords.first() {
                let line_coords: Vec<Coord<f64>> = ring
                    .iter()
                    .map(|&(cx, cy)| to_lonlat(cx as f64, cy as f64))
                    .collect();
                if line_coords.len() >= 2 {
                    Geometry::Line(LineString::new(line_coords))
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        3 => {
            // Polygon
            let mut rings = Vec::new();
            for ring in &coords {
                let ring_coords: Vec<Coord<f64>> = ring
                    .iter()
                    .map(|&(cx, cy)| to_lonlat(cx as f64, cy as f64))
                    .collect();
                if ring_coords.len() >= 3 {
                    rings.push(LineString::new(ring_coords));
                }
            }
            if rings.is_empty() {
                return None;
            }
            let exterior = rings.remove(0);
            Geometry::Polygon(Polygon::new(exterior, rings))
        }
        _ => return None,
    };

    Some(DecodedFeature {
        layer: layer_name.to_string(),
        class,
        name: feat_name,
        tags: extra_tags,
        geometry,
    })
}

/// Decode MVT geometry commands into rings of (x, y) coordinates.
fn decode_geometry_commands(data: &[u8]) -> Vec<Vec<(i32, i32)>> {
    let mut vals = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let (v, new_pos) = read_varint(data, pos);
        vals.push(v as u32);
        pos = new_pos;
    }

    let mut rings: Vec<Vec<(i32, i32)>> = Vec::new();
    let mut current_ring: Vec<(i32, i32)> = Vec::new();
    let mut cx: i32 = 0;
    let mut cy: i32 = 0;
    let mut i = 0;

    while i < vals.len() {
        let cmd = vals[i];
        i += 1;
        let cmd_id = cmd & 0x7;
        let count = (cmd >> 3) as usize;

        match cmd_id {
            1 => {
                // MoveTo
                if !current_ring.is_empty() {
                    rings.push(std::mem::take(&mut current_ring));
                }
                for _ in 0..count {
                    if i + 1 >= vals.len() {
                        break;
                    }
                    let dx = zigzag_decode(vals[i]);
                    let dy = zigzag_decode(vals[i + 1]);
                    i += 2;
                    cx += dx;
                    cy += dy;
                    current_ring.push((cx, cy));
                }
            }
            2 => {
                // LineTo
                for _ in 0..count {
                    if i + 1 >= vals.len() {
                        break;
                    }
                    let dx = zigzag_decode(vals[i]);
                    let dy = zigzag_decode(vals[i + 1]);
                    i += 2;
                    cx += dx;
                    cy += dy;
                    current_ring.push((cx, cy));
                }
            }
            7 => {
                // ClosePath
                if let Some(&first) = current_ring.first() {
                    current_ring.push(first);
                }
            }
            _ => {}
        }
    }

    if !current_ring.is_empty() {
        rings.push(current_ring);
    }

    rings
}

fn zigzag_decode(n: u32) -> i32 {
    ((n >> 1) as i32) ^ -((n & 1) as i32)
}

fn decode_value(data: &[u8]) -> String {
    let mut pos = 0;
    while pos < data.len() {
        let (tag, new_pos) = read_varint(data, pos);
        let field = tag >> 3;
        let wire = tag & 7;
        pos = new_pos;

        match (field, wire) {
            (1, 2) => {
                // String value
                let (len, new_pos) = read_varint(data, pos);
                pos = new_pos;
                return String::from_utf8_lossy(&data[pos..pos + len as usize]).to_string();
            }
            (2, 5) => {
                // Float
                if pos + 4 <= data.len() {
                    let v = f32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                    return v.to_string();
                }
                pos += 4;
            }
            (3, 1) => {
                // Double
                if pos + 8 <= data.len() {
                    let v = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
                    return v.to_string();
                }
                pos += 8;
            }
            (4, 0) | (5, 0) | (6, 0) => {
                // Int/UInt/SInt
                let (v, _new_pos) = read_varint(data, pos);
                return v.to_string();
            }
            (7, 0) => {
                // Bool
                let (v, _new_pos) = read_varint(data, pos);
                return if v != 0 { "true" } else { "false" }.to_string();
            }
            _ => {
                if wire == 0 {
                    let (_, new_pos) = read_varint(data, pos);
                    pos = new_pos;
                } else if wire == 2 {
                    let (len, new_pos) = read_varint(data, pos);
                    pos = new_pos + len as usize;
                } else if wire == 5 {
                    pos += 4;
                } else if wire == 1 {
                    pos += 8;
                } else {
                    break;
                }
            }
        }
    }
    String::new()
}

fn read_varint(data: &[u8], mut pos: usize) -> (u64, usize) {
    let mut result: u64 = 0;
    let mut shift = 0;
    while pos < data.len() {
        let b = data[pos];
        pos += 1;
        result |= ((b & 0x7f) as u64) << shift;
        shift += 7;
        if b < 0x80 {
            break;
        }
    }
    (result, pos)
}
