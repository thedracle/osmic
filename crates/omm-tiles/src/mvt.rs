use mvt::{GeomData, GeomEncoder, GeomType, Tile};
use omm_core::geometry::Geometry;
use omm_osm::feature::Feature;
use omm_osm::tags::{TagStore, WellKnownKey};

use crate::coord::TileTransform;

/// Encode a geometry into MVT GeomData using the given tile transform.
pub fn encode_geometry(
    geometry: &Geometry,
    transform: &TileTransform,
) -> Result<GeomData, mvt::Error> {
    match geometry {
        Geometry::Point(p) => {
            let (x, y) = transform.lon_lat_to_tile(p.x(), p.y());
            GeomEncoder::new(GeomType::Point).point(x, y)?.encode()
        }
        Geometry::Line(ls) => {
            let mut encoder = GeomEncoder::new(GeomType::Linestring);
            for coord in ls.coords() {
                let (x, y) = transform.lon_lat_to_tile(coord.x, coord.y);
                encoder.add_point(x, y)?;
            }
            encoder.encode()
        }
        Geometry::Polygon(poly) => {
            let mut encoder = GeomEncoder::new(GeomType::Polygon);
            let rings: Vec<_> = std::iter::once(poly.exterior())
                .chain(poly.interiors())
                .collect();
            for (i, ring) in rings.iter().enumerate() {
                for coord in ring.coords() {
                    let (x, y) = transform.lon_lat_to_tile(coord.x, coord.y);
                    encoder.add_point(x, y)?;
                }
                if i < rings.len() - 1 {
                    encoder.complete_geom()?;
                }
            }
            encoder.encode()
        }
        Geometry::MultiPolygon(mp) => {
            let mut encoder = GeomEncoder::new(GeomType::Polygon);
            let total_polys = mp.0.len();
            for (pi, poly) in mp.iter().enumerate() {
                let rings: Vec<_> = std::iter::once(poly.exterior())
                    .chain(poly.interiors())
                    .collect();
                for (ri, ring) in rings.iter().enumerate() {
                    for coord in ring.coords() {
                        let (x, y) = transform.lon_lat_to_tile(coord.x, coord.y);
                        encoder.add_point(x, y)?;
                    }
                    let is_last = pi == total_polys - 1 && ri == rings.len() - 1;
                    if !is_last {
                        encoder.complete_geom()?;
                    }
                }
            }
            encoder.encode()
        }
    }
}

/// Build an MVT tile from grouped features.
///
/// `layer_features` maps MVT layer names to slices of (feature_index, &Feature).
/// Returns encoded MVT bytes, or None if the tile is empty.
pub fn build_tile(
    extent: u32,
    transform: &TileTransform,
    layer_features: &[(&str, &[(&Feature, usize)])],
    tag_store: &TagStore,
) -> Option<Vec<u8>> {
    let mut tile = Tile::new(extent);
    let name_key = tag_store.well_known(WellKnownKey::Name);

    for &(layer_name, features) in layer_features {
        if features.is_empty() {
            continue;
        }

        let mut layer = tile.create_layer(layer_name);

        for &(feature, _idx) in features {
            match encode_geometry(&feature.geometry, transform) {
                Ok(geom_data) => {
                    let mut mvt_feature = layer.into_feature(geom_data);
                    mvt_feature.set_id(feature.id as u64);
                    mvt_feature.add_tag_string("class", feature.kind.class_name());

                    if let Some(name_val) = feature.tags.get(name_key) {
                        mvt_feature.add_tag_string("name", tag_store.resolve(name_val));
                    }

                    layer = mvt_feature.into_layer();
                }
                Err(_) => continue,
            }
        }

        if layer.num_features() > 0 {
            let _ = tile.add_layer(layer);
        }
    }

    if tile.num_layers() == 0 {
        return None;
    }

    tile.to_bytes().ok()
}
