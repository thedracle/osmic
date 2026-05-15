//! TPAK bundle generator — packs all tiles for a region into a single file.
//!
//! Format:
//!   Header (20 bytes): magic(4) + tile_count(4) + version(4) + bbox(4×i16=8)
//!   Index (14 bytes per tile): grid_lat(f32) + grid_lon(f32) + offset(u32) + size(u16)
//!   Tile data: concatenated TMAP v3 blobs

use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use osmic_compact::AreaBuilder;
use osmic_core::BBox;
use osmic_index::feature_index::FeatureIndex;
use osmic_osm::feature::Feature;
use osmic_osm::tags::TagStore;
use tracing::info;

const MAGIC: &[u8; 4] = b"TPAK";
const HEADER_SIZE: usize = 20;
const INDEX_ENTRY_SIZE: usize = 14;

pub struct BundleConfig {
    pub region_bbox: BBox,
    pub grid_step: f64,
    pub display_width: u8,
    pub display_height: u8,
    pub contour_interval: u8,
    pub hgt_dir: String,
}

/// Generate a TPAK bundle for all grid cells in a region.
pub fn generate_bundle(
    features: &[Feature],
    tag_store: &Arc<TagStore>,
    feature_index: &FeatureIndex,
    config: &BundleConfig,
) -> Vec<u8> {
    let bb = &config.region_bbox;
    let step = config.grid_step;

    let lat_start = (bb.min_lat / step).floor() * step;
    let lon_start = (bb.min_lon / step).floor() * step;

    let builder = AreaBuilder {
        display_width: config.display_width,
        display_height: config.display_height,
        contour_interval: config.contour_interval,
    };

    // Generate all non-empty tiles
    let mut tiles: Vec<(f32, f32, Vec<u8>)> = Vec::new();
    let mut lat = lat_start;
    let mut generated = 0u32;
    let mut skipped = 0u32;

    while lat < bb.max_lat {
        let mut lon = lon_start;
        while lon < bb.max_lon {
            let tile_bbox = BBox {
                min_lon: lon,
                min_lat: lat,
                max_lon: lon + step,
                max_lat: lat + step,
            };

            let indices = feature_index.query_bbox(&tile_bbox);
            if indices.is_empty() {
                skipped += 1;
                lon += step;
                continue;
            }

            // Contours from HGT if available
            let hgt_name = crate::hgt_filename(lat, lon);
            let hgt_path = Path::new(&config.hgt_dir).join(&hgt_name);
            let contours = if hgt_path.exists() {
                osmic_compact::contour::generate_contours(
                    &hgt_path,
                    &tile_bbox,
                    config.contour_interval as u16,
                )
                .unwrap_or_default()
            } else {
                vec![]
            };

            let blob = builder.build_subset(features, &indices, &tile_bbox, tag_store, &contours);

            if blob.len() > 32 {
                tiles.push((lat as f32, lon as f32, blob));
                generated += 1;
                if generated % 100 == 0 {
                    info!(generated, skipped, "tile generation progress");
                }
            } else {
                skipped += 1;
            }

            lon += step;
        }
        lat += step;
    }

    info!(generated, skipped, "tile generation complete");

    // Sort by grid key for binary search
    tiles.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });

    // Compute data offsets
    let tile_count = tiles.len();
    let data_start = HEADER_SIZE + tile_count * INDEX_ENTRY_SIZE;
    let mut offsets: Vec<(f32, f32, u32, u16)> = Vec::with_capacity(tile_count);
    let mut pos = data_start as u32;

    for (lat, lon, blob) in &tiles {
        let size = blob.len() as u16;
        offsets.push((*lat, *lon, pos, size));
        pos += size as u32;
    }

    // Build output
    let total_size = pos as usize;
    let mut out = Vec::with_capacity(total_size);

    // Header (20 bytes)
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&(tile_count as u32).to_le_bytes());
    let version = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&((bb.min_lat * 100.0) as i16).to_le_bytes());
    out.extend_from_slice(&((bb.min_lon * 100.0) as i16).to_le_bytes());
    out.extend_from_slice(&((bb.max_lat * 100.0) as i16).to_le_bytes());
    out.extend_from_slice(&((bb.max_lon * 100.0) as i16).to_le_bytes());

    // Index
    for (lat, lon, offset, size) in &offsets {
        out.extend_from_slice(&lat.to_le_bytes());
        out.extend_from_slice(&lon.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
    }

    // Tile data
    for (_, _, blob) in &tiles {
        out.extend_from_slice(blob);
    }

    info!(
        tiles = tile_count,
        bytes = out.len(),
        kb = out.len() / 1024,
        "TPAK bundle complete"
    );

    out
}
