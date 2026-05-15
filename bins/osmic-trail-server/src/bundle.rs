//! TPAK bundle generator — packs all tiles for a region into a single file.
//!
//! Two modes:
//!   - `generate_bundle`: in-memory (for small regions, returns Vec<u8>)
//!   - `generate_bundle_to_file`: streaming to disk (for large regions, O(1) memory)
//!
//! Format:
//!   Header (20 bytes): magic(4) + tile_count(4) + version(4) + bbox(4×i16=8)
//!   Index (14 bytes per tile): grid_lat(f32) + grid_lon(f32) + offset(u32) + size(u16)
//!   Tile data: concatenated TMAP v3 blobs

use std::io::{self, Seek, SeekFrom, Write};
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

/// Index entry collected during tile generation.
struct TileEntry {
    grid_lat: f32,
    grid_lon: f32,
    offset: u32,
    size: u16,
}

/// Iterate all grid cells in the bbox and call a visitor for each non-empty tile.
/// The visitor receives (grid_lat, grid_lon, tile_blob) and returns whether to continue.
fn for_each_tile(
    features: &[Feature],
    tag_store: &Arc<TagStore>,
    feature_index: &FeatureIndex,
    config: &BundleConfig,
    mut visitor: impl FnMut(f32, f32, Vec<u8>) -> bool,
) {
    let bb = &config.region_bbox;
    let step = config.grid_step;
    let lat_start = (bb.min_lat / step).floor() * step;
    let lon_start = (bb.min_lon / step).floor() * step;

    let builder = AreaBuilder {
        display_width: config.display_width,
        display_height: config.display_height,
        contour_interval: config.contour_interval,
    };

    let mut generated = 0u32;
    let mut lat = lat_start;

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
            if !indices.is_empty() {
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

                let blob = builder.build_subset(
                    features, &indices, &tile_bbox, tag_store, &contours,
                );

                if blob.len() > 32 {
                    generated += 1;
                    if generated % 100 == 0 {
                        info!(generated, "tile generation progress");
                    }
                    if !visitor(lat as f32, lon as f32, blob) {
                        return;
                    }
                }
            }

            lon += step;
        }
        lat += step;
    }

    info!(generated, "tile generation complete");
}

fn write_header(w: &mut impl Write, tile_count: u32, bb: &BBox) -> io::Result<()> {
    w.write_all(MAGIC)?;
    w.write_all(&tile_count.to_le_bytes())?;
    let version = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    w.write_all(&version.to_le_bytes())?;
    w.write_all(&((bb.min_lat * 100.0) as i16).to_le_bytes())?;
    w.write_all(&((bb.min_lon * 100.0) as i16).to_le_bytes())?;
    w.write_all(&((bb.max_lat * 100.0) as i16).to_le_bytes())?;
    w.write_all(&((bb.max_lon * 100.0) as i16).to_le_bytes())?;
    Ok(())
}

fn write_index_entry(w: &mut impl Write, entry: &TileEntry) -> io::Result<()> {
    w.write_all(&entry.grid_lat.to_le_bytes())?;
    w.write_all(&entry.grid_lon.to_le_bytes())?;
    w.write_all(&entry.offset.to_le_bytes())?;
    w.write_all(&entry.size.to_le_bytes())?;
    Ok(())
}

/// Generate a TPAK bundle, streaming directly to a file.
/// Peak memory: one tile (~15KB) regardless of total bundle size.
pub fn generate_bundle_to_file(
    output_path: &Path,
    features: &[Feature],
    tag_store: &Arc<TagStore>,
    feature_index: &FeatureIndex,
    config: &BundleConfig,
) -> io::Result<(u32, u64)> {
    let bb = &config.region_bbox;
    let step = config.grid_step;

    // Estimate max tiles to reserve index space
    let lat_cells = ((bb.max_lat - bb.min_lat) / step).ceil() as usize + 1;
    let lon_cells = ((bb.max_lon - bb.min_lon) / step).ceil() as usize + 1;
    let max_tiles = lat_cells * lon_cells;
    let reserved_index_size = max_tiles * INDEX_ENTRY_SIZE;
    let data_start = HEADER_SIZE + reserved_index_size;

    let mut file = std::fs::File::create(output_path)?;

    // Write placeholder header
    write_header(&mut file, 0, bb)?;

    // Write placeholder index (zeroed)
    let zeros = vec![0u8; reserved_index_size];
    file.write_all(&zeros)?;

    // Generate tiles and append to file, collecting index entries
    let mut index: Vec<TileEntry> = Vec::new();
    let mut write_offset = data_start as u32;

    for_each_tile(features, tag_store, feature_index, config, |lat, lon, blob| {
        let size = blob.len() as u16;
        index.push(TileEntry {
            grid_lat: lat,
            grid_lon: lon,
            offset: write_offset,
            size,
        });
        if let Err(e) = file.write_all(&blob) {
            tracing::error!(error = %e, "failed to write tile data");
            return false;
        }
        write_offset += size as u32;
        true
    });

    // Sort index by grid key
    index.sort_by(|a, b| {
        a.grid_lat
            .partial_cmp(&b.grid_lat)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.grid_lon
                    .partial_cmp(&b.grid_lon)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    let tile_count = index.len() as u32;

    // Seek back and write the real header with correct tile count
    file.seek(SeekFrom::Start(0))?;
    write_header(&mut file, tile_count, bb)?;

    // Write the real index (sorted)
    for entry in &index {
        write_index_entry(&mut file, entry)?;
    }

    // If we used fewer tiles than reserved, truncate the file to remove
    // the gap between the end of the actual index and the tile data.
    // The data offsets in the index already point to the right places
    // (they were computed based on data_start with max_tiles reserve),
    // so we need to compact.

    // Actually, since offsets in the index already point to the data written
    // at data_start + ..., and the index is shorter than reserved, there's a
    // gap of (max_tiles - tile_count) * 14 bytes between the index and data.
    // The offsets are still correct because data was written at data_start.
    // The gap is just wasted space. For simplicity, leave it — the phone
    // can handle it. For production, we'd do a second pass to compact.

    let file_size = file.seek(SeekFrom::End(0))?;

    info!(
        tiles = tile_count,
        bytes = file_size,
        kb = file_size / 1024,
        "TPAK bundle streamed to disk"
    );

    Ok((tile_count, file_size))
}

/// Generate a TPAK bundle in memory (for small regions).
/// For large regions, use `generate_bundle_to_file` instead.
pub fn generate_bundle(
    features: &[Feature],
    tag_store: &Arc<TagStore>,
    feature_index: &FeatureIndex,
    config: &BundleConfig,
) -> Vec<u8> {
    let bb = &config.region_bbox;

    let mut tiles: Vec<(f32, f32, Vec<u8>)> = Vec::new();

    for_each_tile(features, tag_store, feature_index, config, |lat, lon, blob| {
        tiles.push((lat, lon, blob));
        true
    });

    tiles.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });

    let tile_count = tiles.len();
    let data_start = HEADER_SIZE + tile_count * INDEX_ENTRY_SIZE;
    let mut offsets: Vec<TileEntry> = Vec::with_capacity(tile_count);
    let mut pos = data_start as u32;

    for (lat, lon, blob) in &tiles {
        offsets.push(TileEntry {
            grid_lat: *lat,
            grid_lon: *lon,
            offset: pos,
            size: blob.len() as u16,
        });
        pos += blob.len() as u32;
    }

    let mut out = Vec::with_capacity(pos as usize);

    // Header
    let mut header = Vec::with_capacity(HEADER_SIZE);
    write_header(&mut header, tile_count as u32, bb).unwrap();
    out.extend_from_slice(&header);

    // Index
    for entry in &offsets {
        let mut idx = Vec::with_capacity(INDEX_ENTRY_SIZE);
        write_index_entry(&mut idx, entry).unwrap();
        out.extend_from_slice(&idx);
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
