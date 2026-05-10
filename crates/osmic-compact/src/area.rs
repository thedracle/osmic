//! AreaBuilder: produces a single CompactTrailBlob for a geographic bounding box.
//!
//! Takes OSM features (and optionally contour features), filters to trail-relevant
//! kinds, simplifies, projects to pixel space, and encodes into the compact binary format.

use geo::Simplify;
use osmic_core::{BBox, Geometry};
use osmic_osm::feature::Feature;
use osmic_osm::tags::TagStore;

use crate::encoder::{encode_geometry, encode_poi, PixelProjector};
use crate::filter::{is_poi, is_trail_relevant, to_poi_type};
use crate::format::{CompactHeader, VERSION};

/// Pixel grid size for POI deduplication. POIs of the same type that hash to the
/// same `(px / SIZE, py / SIZE)` cell collapse to a single marker. 6 px on a
/// 176 px display ≈ a 30×30 grid — coarse enough to absorb tightly-clustered
/// OSM features (e.g. 14 PicnicSite nodes at one campground) without merging
/// genuinely separate landmarks.
const POI_DEDUP_CELL_PX: u8 = 6;

/// Builder for compact trail area blobs.
pub struct AreaBuilder {
    pub display_width: u8,
    pub display_height: u8,
    pub contour_interval: u8,
}

impl Default for AreaBuilder {
    fn default() -> Self {
        Self {
            display_width: 176,
            display_height: 176,
            contour_interval: 20,
        }
    }
}

impl AreaBuilder {
    pub fn new(width: u8, height: u8) -> Self {
        Self {
            display_width: width,
            display_height: height,
            ..Default::default()
        }
    }

    /// Build a compact trail blob from OSM features within a bounding box.
    ///
    /// `features` — all features from PBF processing (will be filtered)
    /// `bbox` — the geographic area to encode
    /// `tag_store` — for resolving feature names
    /// `contour_features` — optional pre-generated contour features
    pub fn build(
        &self,
        features: &[Feature],
        bbox: &BBox,
        tag_store: &TagStore,
        contour_features: &[Feature],
    ) -> Vec<u8> {
        let projector = PixelProjector::new(bbox, self.display_width, self.display_height);

        // Simplification tolerance: tuned for pixel dimensions.
        // At 176px across a typical trail area (~5km), 1 pixel ≈ 28m ≈ 0.00025°.
        // Use ~2 pixels as tolerance to aggressively simplify.
        let lon_range = bbox.max_lon - bbox.min_lon;
        let tolerance = lon_range / (self.display_width as f64 / 2.0);

        let name_key = tag_store.get("name");
        let ele_key = tag_store.get("ele");

        let mut feature_buf = Vec::new();
        let mut poi_buf = Vec::new();
        let mut feature_count: u32 = 0;
        let mut poi_count: u32 = 0;
        let mut poi_cells: Vec<(u8, u8, u8)> = Vec::new();

        // Process OSM features + contour features together
        let all_features = features.iter().chain(contour_features.iter());

        for feature in all_features {
            if !is_trail_relevant(&feature.kind) {
                continue;
            }
            if !feature_bbox_intersects(&feature.geometry, bbox) {
                continue;
            }

            if is_poi(&feature.kind) {
                // Encode as POI — but only if it has a name. An unlabeled dot on a
                // 176×176 watch is just noise; OSM has many small Cliff/Information
                // POIs without names that aren't useful at this scale.
                if let Geometry::Point(pt) = &feature.geometry {
                    let name = name_key
                        .and_then(|k| feature.tags.get(k))
                        .map(|v| tag_store.resolve(v))
                        .unwrap_or("");
                    if name.is_empty() {
                        continue;
                    }
                    let (px, py) = projector.project(pt.x(), pt.y());
                    let cell = (
                        px / POI_DEDUP_CELL_PX,
                        py / POI_DEDUP_CELL_PX,
                        to_poi_type(&feature.kind) as u8,
                    );
                    if poi_cells.contains(&cell) {
                        continue;
                    }
                    let elevation = ele_key
                        .and_then(|k| feature.tags.get(k))
                        .map(|v| tag_store.resolve(v))
                        .and_then(parse_elevation_meters)
                        .unwrap_or(0);
                    encode_poi(
                        &feature.kind,
                        pt.x(),
                        pt.y(),
                        elevation,
                        name,
                        &projector,
                        &mut poi_buf,
                    );
                    poi_cells.push(cell);
                    poi_count += 1;
                }
                continue;
            }

            // Simplify geometry
            let simplified = simplify_geometry(&feature.geometry, tolerance);

            // Clip to bbox (simple rejection for now — features already filtered by bbox)
            let count = encode_geometry(&feature.kind, &simplified, &projector, &mut feature_buf);
            feature_count += count;
        }

        // Build the final blob
        let header = CompactHeader {
            version: VERSION,
            display_width: self.display_width,
            display_height: self.display_height,
            contour_interval: self.contour_interval,
            bbox_min_lon: (bbox.min_lon * 1_000_000.0) as i32,
            bbox_min_lat: (bbox.min_lat * 1_000_000.0) as i32,
            bbox_max_lon: (bbox.max_lon * 1_000_000.0) as i32,
            bbox_max_lat: (bbox.max_lat * 1_000_000.0) as i32,
            feature_count: feature_count.min(u16::MAX as u32) as u16,
            poi_count: poi_count.min(u16::MAX as u32) as u16,
        };

        let mut blob = Vec::with_capacity(32 + feature_buf.len() + poi_buf.len());
        blob.extend_from_slice(&header.to_bytes());
        blob.extend_from_slice(&feature_buf);
        blob.extend_from_slice(&poi_buf);

        tracing::info!(
            features = feature_count,
            pois = poi_count,
            bytes = blob.len(),
            "compact trail blob built"
        );

        blob
    }
}

/// Parse an OSM `ele` tag value into integer meters, clamped to `u16`.
///
/// OSM convention: a bare number is meters; values may be float ("3266.5") or
/// have a unit suffix ("3266 m", "10700 ft"). Negative elevations clamp to 0.
fn parse_elevation_meters(s: &str) -> Option<u16> {
    let s = s.trim();
    let (num, unit) = match s.split_once(|c: char| c.is_whitespace() || c == ',') {
        Some((n, u)) => (n, u.trim()),
        None => (s, ""),
    };
    let val: f64 = num.parse().ok()?;
    let meters = match unit.to_ascii_lowercase().as_str() {
        "" | "m" => val,
        "ft" | "feet" | "'" => val * 0.3048,
        _ => return None,
    };
    if !meters.is_finite() || meters <= 0.0 {
        return Some(0);
    }
    Some(meters.round().min(u16::MAX as f64) as u16)
}

/// Check if a feature's geometry bbox intersects the target area.
fn feature_bbox_intersects(geometry: &Geometry, bbox: &BBox) -> bool {
    let fbbox = geometry.bbox();
    fbbox.min_lon <= bbox.max_lon
        && fbbox.max_lon >= bbox.min_lon
        && fbbox.min_lat <= bbox.max_lat
        && fbbox.max_lat >= bbox.min_lat
}

/// Simplify a geometry using Douglas-Peucker.
fn simplify_geometry(geometry: &Geometry, tolerance: f64) -> Geometry {
    match geometry {
        Geometry::Point(_) => geometry.clone(),
        Geometry::Line(line) => {
            let simplified = line.simplify(tolerance);
            if simplified.0.len() >= 2 {
                Geometry::Line(simplified)
            } else {
                geometry.clone()
            }
        }
        Geometry::Polygon(poly) => {
            let ext = poly.exterior().simplify(tolerance);
            if ext.0.len() >= 4 {
                Geometry::Polygon(geo_types::Polygon::new(ext, vec![]))
            } else {
                geometry.clone()
            }
        }
        Geometry::MultiPolygon(mp) => {
            let polys: Vec<geo_types::Polygon<f64>> = mp
                .iter()
                .filter_map(|p| {
                    let ext = p.exterior().simplify(tolerance);
                    if ext.0.len() >= 4 {
                        Some(geo_types::Polygon::new(ext, vec![]))
                    } else {
                        None
                    }
                })
                .collect();
            Geometry::MultiPolygon(geo_types::MultiPolygon::new(polys))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_types::LineString;
    use osmic_osm::feature::HighwayKind;
    use osmic_osm::tags::{TagStore, Tags};

    fn test_bbox() -> BBox {
        BBox {
            min_lon: -111.80,
            min_lat: 40.60,
            max_lon: -111.75,
            max_lat: 40.65,
        }
    }

    fn make_line_feature(kind: osmic_osm::FeatureKind, coords: Vec<(f64, f64)>) -> Feature {
        Feature {
            id: 1,
            kind,
            geometry: Geometry::Line(LineString::from(coords)),
            tags: Tags::new(),
        }
    }

    #[test]
    fn builds_blob_with_header() {
        let tag_store = TagStore::new();
        let builder = AreaBuilder::default();
        let features = vec![make_line_feature(
            osmic_osm::FeatureKind::Highway(HighwayKind::Path),
            vec![(-111.79, 40.61), (-111.78, 40.62), (-111.77, 40.63)],
        )];
        let blob = builder.build(&features, &test_bbox(), &tag_store, &[]);
        assert!(blob.len() >= 32); // at least header
        assert_eq!(&blob[0..4], b"TMAP");
        let header = CompactHeader::from_bytes(&blob).unwrap();
        assert_eq!(header.feature_count, 1);
        assert_eq!(header.poi_count, 0);
    }

    #[test]
    fn filters_irrelevant_features() {
        let tag_store = TagStore::new();
        let builder = AreaBuilder::default();
        let features = vec![
            make_line_feature(
                osmic_osm::FeatureKind::Highway(HighwayKind::Path),
                vec![(-111.79, 40.61), (-111.78, 40.62)],
            ),
            // Building should be filtered out
            Feature {
                id: 2,
                kind: osmic_osm::FeatureKind::Building(osmic_osm::feature::BuildingKind::Yes),
                geometry: Geometry::Polygon(geo_types::Polygon::new(
                    LineString::from(vec![
                        (-111.78, 40.62),
                        (-111.77, 40.62),
                        (-111.77, 40.63),
                        (-111.78, 40.63),
                        (-111.78, 40.62),
                    ]),
                    vec![],
                )),
                tags: Tags::new(),
            },
        ];
        let blob = builder.build(&features, &test_bbox(), &tag_store, &[]);
        let header = CompactHeader::from_bytes(&blob).unwrap();
        assert_eq!(header.feature_count, 1); // only the path
    }

    fn make_named_poi(
        id: i64,
        kind: osmic_osm::FeatureKind,
        lon: f64,
        lat: f64,
        name: &str,
        tag_store: &TagStore,
    ) -> Feature {
        let name_key = tag_store.intern_key("name");
        let name_val = tag_store.intern_value(name);
        let mut tags = Tags::new();
        tags.push(name_key, name_val);
        Feature {
            id,
            kind,
            geometry: Geometry::Point(geo_types::Point::new(lon, lat)),
            tags,
        }
    }

    #[test]
    fn clustered_same_type_pois_collapse_to_one() {
        // Mirrors the OSM Wasatch picnic-site cluster: same-type POIs within
        // a few pixels of each other after projection should collapse into a
        // single grid cell.
        let tag_store = TagStore::new();
        let kind = osmic_osm::FeatureKind::Tourism(osmic_osm::feature::TourismKind::PicnicSite);
        // Six POIs spread over ~0.5 px, all landing in the same dedup cell.
        let features = vec![
            make_named_poi(1, kind.clone(), -111.77400, 40.62500, "Site 1", &tag_store),
            make_named_poi(2, kind.clone(), -111.77402, 40.62501, "Site 2", &tag_store),
            make_named_poi(3, kind.clone(), -111.77404, 40.62502, "Site 3", &tag_store),
            make_named_poi(4, kind.clone(), -111.77406, 40.62503, "Site 4", &tag_store),
            make_named_poi(5, kind.clone(), -111.77408, 40.62504, "Site 5", &tag_store),
            make_named_poi(6, kind, -111.77410, 40.62505, "Site 6", &tag_store),
        ];
        let builder = AreaBuilder::default();
        let blob = builder.build(&features, &test_bbox(), &tag_store, &[]);
        let header = CompactHeader::from_bytes(&blob).unwrap();
        assert_eq!(header.poi_count, 1, "clustered same-type POIs should collapse");
    }

    #[test]
    fn distinct_types_at_same_pixel_are_kept() {
        // A peak and a viewpoint sharing a pixel are different things —
        // dedup must not collapse across types.
        let tag_store = TagStore::new();
        let features = vec![
            make_named_poi(
                1,
                osmic_osm::FeatureKind::Natural(osmic_osm::feature::NaturalKind::Peak),
                -111.775,
                40.625,
                "Mt Test",
                &tag_store,
            ),
            make_named_poi(
                2,
                osmic_osm::FeatureKind::Tourism(osmic_osm::feature::TourismKind::Viewpoint),
                -111.775,
                40.625,
                "Lookout",
                &tag_store,
            ),
        ];
        let builder = AreaBuilder::default();
        let blob = builder.build(&features, &test_bbox(), &tag_store, &[]);
        let header = CompactHeader::from_bytes(&blob).unwrap();
        assert_eq!(header.poi_count, 2);
    }

    #[test]
    fn parses_plain_meters() {
        assert_eq!(parse_elevation_meters("3266"), Some(3266));
        assert_eq!(parse_elevation_meters("3266.5"), Some(3267));
        assert_eq!(parse_elevation_meters("3266 m"), Some(3266));
    }

    #[test]
    fn parses_feet_unit() {
        // 10700 ft ≈ 3261 m
        assert_eq!(parse_elevation_meters("10700 ft"), Some(3261));
    }

    #[test]
    fn rejects_bad_elevation() {
        assert_eq!(parse_elevation_meters(""), None);
        assert_eq!(parse_elevation_meters("approx 3000"), None);
        assert_eq!(parse_elevation_meters("3000 furlongs"), None);
    }

    #[test]
    fn clamps_negative_elevation_to_zero() {
        assert_eq!(parse_elevation_meters("-5"), Some(0));
    }

    #[test]
    fn blob_size_is_compact() {
        let tag_store = TagStore::new();
        let builder = AreaBuilder::default();
        // Create several trail features
        let features: Vec<Feature> = (0..50)
            .map(|i| {
                let lon = -111.79 + (i as f64 * 0.001);
                make_line_feature(
                    osmic_osm::FeatureKind::Highway(HighwayKind::Path),
                    vec![(lon, 40.61), (lon + 0.005, 40.615), (lon + 0.01, 40.62)],
                )
            })
            .collect();
        let blob = builder.build(&features, &test_bbox(), &tag_store, &[]);
        // 50 features with 3 points each should be well under 5KB
        assert!(blob.len() < 5000, "blob too large: {} bytes", blob.len());
    }
}
