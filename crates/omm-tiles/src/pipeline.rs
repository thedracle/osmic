use std::collections::HashMap;
use std::time::Instant;

use rayon::prelude::*;
use tracing::info;

use omm_core::bbox::BBox;
use omm_core::error::OmmResult;
use omm_core::tile::{TileCoord, Zoom};
use omm_geo::projection::bbox_to_tile_range;
use omm_osm::feature::Feature;
use omm_osm::tags::TagStore;

use crate::coord::TileTransform;
use crate::mvt;

/// Configuration for tile generation.
#[derive(Debug, Clone)]
pub struct TileGeneratorConfig {
    pub min_zoom: u8,
    pub max_zoom: u8,
    pub extent: u32,
    pub batch_size: usize,
}

impl Default for TileGeneratorConfig {
    fn default() -> Self {
        Self {
            min_zoom: 0,
            max_zoom: 14,
            extent: 4096,
            batch_size: 10_000,
        }
    }
}

/// Parallel tile generator.
///
/// For each zoom level, computes which tiles contain features (via feature-to-tile
/// mapping), then generates those tiles in parallel with rayon.
pub struct TileGenerator<'a> {
    features: &'a [Feature],
    tag_store: &'a TagStore,
    data_bbox: BBox,
    config: TileGeneratorConfig,
}

impl<'a> TileGenerator<'a> {
    pub fn new(
        features: &'a [Feature],
        tag_store: &'a TagStore,
        data_bbox: BBox,
        config: TileGeneratorConfig,
    ) -> Self {
        Self {
            features,
            tag_store,
            data_bbox,
            config,
        }
    }

    /// Generate all tiles and write to a PMTiles archive via the callback.
    ///
    /// The callback receives (TileCoord, &[u8]) for each generated tile.
    /// Returns total number of tiles generated.
    pub fn generate_all<F>(&self, mut write_tile: F) -> OmmResult<u64>
    where
        F: FnMut(TileCoord, &[u8]) -> OmmResult<()>,
    {
        let mut total_tiles = 0u64;
        let total_start = Instant::now();

        for z in self.config.min_zoom..=self.config.max_zoom {
            let zoom_start = Instant::now();

            // Step 1: Collect which tiles have features at this zoom
            let tile_features = self.collect_tile_features(z);
            let occupied_count = tile_features.len();

            if occupied_count == 0 {
                info!(zoom = z, "No tiles at this zoom, skipping");
                continue;
            }

            info!(
                zoom = z,
                occupied_tiles = occupied_count,
                "Generating tiles"
            );

            // Step 2: Generate tiles in batches
            let tile_entries: Vec<_> = tile_features.into_iter().collect();

            for batch in tile_entries.chunks(self.config.batch_size) {
                let tiles: Vec<_> = batch
                    .par_iter()
                    .filter_map(|((x, y), feature_indices)| {
                        let coord = TileCoord::new(*x, *y, Zoom::new(z));
                        self.generate_single_tile(coord, feature_indices)
                            .map(|bytes| (coord, bytes))
                    })
                    .collect();

                for (coord, bytes) in &tiles {
                    write_tile(*coord, bytes)?;
                    total_tiles += 1;
                }
            }

            info!(
                zoom = z,
                tiles = total_tiles,
                elapsed_s = zoom_start.elapsed().as_secs_f64(),
                "Zoom level complete"
            );
        }

        info!(
            total_tiles,
            elapsed_s = total_start.elapsed().as_secs_f64(),
            "Tile generation complete"
        );

        Ok(total_tiles)
    }

    /// Build a map of (tile_x, tile_y) → [feature_indices] for a given zoom.
    fn collect_tile_features(&self, zoom: u8) -> HashMap<(u32, u32), Vec<usize>> {
        let mut tile_map: HashMap<(u32, u32), Vec<usize>> = HashMap::new();

        // Clamp to data bbox tile range to avoid iterating the whole world
        let (range_min_x, range_min_y, range_max_x, range_max_y) =
            bbox_to_tile_range(&self.data_bbox, zoom);

        for (idx, feature) in self.features.iter().enumerate() {
            if feature.kind.min_zoom() > zoom {
                continue;
            }

            let bb = feature.bbox();
            let (min_x, min_y, max_x, max_y) = bbox_to_tile_range(&bb, zoom);

            // Clamp to data bbox range
            let min_x = min_x.max(range_min_x);
            let min_y = min_y.max(range_min_y);
            let max_x = max_x.min(range_max_x);
            let max_y = max_y.min(range_max_y);

            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    tile_map.entry((x, y)).or_default().push(idx);
                }
            }
        }

        tile_map
    }

    /// Generate a single MVT tile from pre-computed feature indices.
    fn generate_single_tile(
        &self,
        coord: TileCoord,
        feature_indices: &[usize],
    ) -> Option<Vec<u8>> {
        let transform = TileTransform::new(&coord, self.config.extent);

        // Group features by MVT layer name
        let mut layer_map: HashMap<&str, Vec<(&Feature, usize)>> = HashMap::new();
        for &idx in feature_indices {
            let feature = &self.features[idx];
            let layer_name = feature.kind.layer_name();
            layer_map.entry(layer_name).or_default().push((feature, idx));
        }

        // Build layer entries for MVT encoding
        let layer_entries: Vec<(&str, Vec<(&Feature, usize)>)> =
            layer_map.into_iter().collect();

        // Convert to the format expected by mvt::build_tile
        let layer_refs: Vec<(&str, &[(&Feature, usize)])> = layer_entries
            .iter()
            .map(|(name, features)| (*name, features.as_slice()))
            .collect();

        mvt::build_tile(self.config.extent, &transform, &layer_refs, self.tag_store)
    }
}
