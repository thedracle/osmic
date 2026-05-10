//! CompactMonoEncoder: projects geographic coordinates to pixel space and
//! delta-encodes them into the compact binary format.

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
        // Invert Y: latitude increases northward but pixel Y increases downward
        let py = (self.height as i16 - 1) - py;
        let px = px.clamp(0, self.width as i16 - 1) as u8;
        let py = py.clamp(0, self.height as i16 - 1) as u8;
        (px, py)
    }
}

/// Encode a line feature as delta-encoded bytes.
/// Returns None if the line has fewer than 2 points after projection.
pub fn encode_line_feature(
    kind: &FeatureKind,
    coords: &[Coord<f64>],
    projector: &PixelProjector,
    out: &mut Vec<u8>,
) -> bool {
    let projected: Vec<(u8, u8)> = coords.iter().map(|c| projector.project(c.x, c.y)).collect();
    let points = delta_encode_points(&projected);
    if points.is_empty() {
        return false;
    }

    let is_polygon = false;
    // A line geometry classified as an area category (e.g. WaterKind::Wetland or
    // a non-closed lake ring) means OSM never gave us a closed polygon. Encode it
    // under the matching line category so the renderer's category logic stays consistent.
    let category = match to_category(kind) {
        FeatureCategory::WaterArea => FeatureCategory::WaterLine,
        c => c,
    };
    let subcategory = to_subcategory(kind);
    let type_byte = encode_type_byte(is_polygon, category, subcategory);

    write_feature(type_byte, &points, out);
    true
}

/// Encode a polygon feature as delta-encoded bytes (exterior ring only).
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
    let points = delta_encode_points(&coords);
    if points.is_empty() {
        return false;
    }

    let is_polygon = true;
    let category = to_category(kind);
    let subcategory = to_subcategory(kind);
    let type_byte = encode_type_byte(is_polygon, category, subcategory);

    write_feature(type_byte, &points, out);
    true
}

/// Encode a POI.
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

/// Delta-encode a list of pixel coordinates.
/// Handles i8 overflow by inserting intermediate points.
/// Returns (first_x, first_y, delta_pairs) or empty vec if too few points.
struct DeltaEncoded {
    first_x: u8,
    first_y: u8,
    deltas: Vec<(i8, i8)>,
}

fn delta_encode_points(points: &[(u8, u8)]) -> Vec<DeltaEncoded> {
    if points.len() < 2 {
        return vec![];
    }

    let mut result = DeltaEncoded {
        first_x: points[0].0,
        first_y: points[0].1,
        deltas: Vec::with_capacity(points.len() - 1),
    };

    let mut prev_x = points[0].0 as i16;
    let mut prev_y = points[0].1 as i16;

    for &(x, y) in &points[1..] {
        let mut dx = x as i16 - prev_x;
        let mut dy = y as i16 - prev_y;

        // Insert intermediate points if delta exceeds i8 range
        while dx.abs() > 127 || dy.abs() > 127 {
            let step_x = dx.clamp(-127, 127) as i8;
            let step_y = dy.clamp(-127, 127) as i8;
            result.deltas.push((step_x, step_y));
            prev_x += step_x as i16;
            prev_y += step_y as i16;
            dx = x as i16 - prev_x;
            dy = y as i16 - prev_y;
        }

        result.deltas.push((dx as i8, dy as i8));
        prev_x = x as i16;
        prev_y = y as i16;
    }

    // Truncate if too many coordinates for a single feature (u8 coord_count)
    if result.deltas.len() > 254 {
        result.deltas.truncate(254);
    }

    vec![result]
}

fn write_feature(type_byte: u8, encoded: &[DeltaEncoded], out: &mut Vec<u8>) {
    for enc in encoded {
        let coord_count = (enc.deltas.len() + 1) as u8; // +1 for first point
        out.push(type_byte);
        out.push(coord_count);
        out.push(enc.first_x);
        out.push(enc.first_y);
        for &(dx, dy) in &enc.deltas {
            out.push(dx as u8);
            out.push(dy as u8);
        }
    }
}

/// Encode a full geometry, dispatching by type. Returns the number of features written.
pub fn encode_geometry(
    kind: &FeatureKind,
    geometry: &Geometry,
    projector: &PixelProjector,
    out: &mut Vec<u8>,
) -> u32 {
    if is_poi(kind) {
        return 0; // POIs are handled separately
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
                // Closed way classified as line (e.g., roundabout)
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
        Geometry::Point(_) => 0, // Points handled as POIs
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
        let (x, y) = proj.project(-111.80, 40.65); // top-left (max lat)
        assert_eq!((x, y), (0, 0));
        let (x, y) = proj.project(-111.75, 40.60); // bottom-right (min lat)
        assert_eq!((x, y), (175, 175));
    }

    #[test]
    fn delta_encode_simple() {
        let points = vec![(10u8, 20u8), (15, 25), (20, 30)];
        let encoded = delta_encode_points(&points);
        assert_eq!(encoded.len(), 1);
        assert_eq!(encoded[0].first_x, 10);
        assert_eq!(encoded[0].first_y, 20);
        assert_eq!(encoded[0].deltas, vec![(5, 5), (5, 5)]);
    }

    #[test]
    fn delta_encode_handles_empty() {
        let points = vec![(10u8, 20u8)];
        let encoded = delta_encode_points(&points);
        assert!(encoded.is_empty());
    }

    #[test]
    fn line_in_water_area_kind_is_demoted_to_water_line() {
        // OSM Wetland is classified as WaterArea by `to_category`, but its
        // is_area() is false — so the upstream pipeline gives us a Line geometry.
        // The encoder should write the line under WaterLine so the type byte's
        // category matches its line/polygon flag.
        let kind = FeatureKind::Water(osmic_osm::feature::WaterKind::Wetland);
        let proj = PixelProjector::new(&test_bbox(), 176, 176);
        let mut buf = Vec::new();
        let coords = vec![
            Coord { x: -111.79, y: 40.61 },
            Coord { x: -111.78, y: 40.62 },
            Coord { x: -111.77, y: 40.63 },
        ];
        let wrote = encode_line_feature(&kind, &coords, &proj, &mut buf);
        assert!(wrote);
        let type_byte = buf[0];
        assert_eq!(type_byte & 0x80, 0, "polygon flag must be unset for a line");
        assert_eq!(
            (type_byte >> 4) & 0x07,
            FeatureCategory::WaterLine as u8,
            "WaterArea kind on a line geometry should write the WaterLine category"
        );
    }

    #[test]
    fn poi_encoding() {
        let kind = FeatureKind::Natural(osmic_osm::feature::NaturalKind::Peak);
        let proj = PixelProjector::new(&test_bbox(), 176, 176);
        let mut buf = Vec::new();
        encode_poi(&kind, -111.775, 40.625, 3200, "Mt Test", &proj, &mut buf);
        assert_eq!(buf[0], PoiType::Peak as u8);
        // Name should be "Mt Test" (7 bytes)
        assert_eq!(buf[5], 7);
        assert_eq!(&buf[6..13], b"Mt Test");
    }
}
