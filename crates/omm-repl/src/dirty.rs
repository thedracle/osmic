use std::collections::HashSet;

use omm_core::bbox::BBox;
use omm_core::tile::TileCoord;
use omm_core::tile::Zoom;
use omm_geo::projection::bbox_to_tile_range;

/// Tracks which tiles need regeneration after feature changes.
pub struct DirtyTileSet {
    dirty: HashSet<TileCoord>,
}

impl DirtyTileSet {
    pub fn new() -> Self {
        Self {
            dirty: HashSet::new(),
        }
    }

    /// Mark all tiles overlapping the given bounding box as dirty
    /// across the specified zoom range.
    pub fn mark_bbox(&mut self, bbox: &BBox, min_zoom: u8, max_zoom: u8) {
        for z in min_zoom..=max_zoom {
            let (min_x, min_y, max_x, max_y) = bbox_to_tile_range(bbox, z);
            for y in min_y..=max_y {
                for x in min_x..=max_x {
                    self.dirty.insert(TileCoord::new(x, y, Zoom::new(z)));
                }
            }
        }
    }

    /// Iterator over all dirty tiles.
    pub fn tiles(&self) -> impl Iterator<Item = &TileCoord> {
        self.dirty.iter()
    }

    /// Number of dirty tiles.
    pub fn len(&self) -> usize {
        self.dirty.len()
    }

    /// Whether there are no dirty tiles.
    pub fn is_empty(&self) -> bool {
        self.dirty.is_empty()
    }

    pub fn clear(&mut self) {
        self.dirty.clear();
    }
}

impl Default for DirtyTileSet {
    fn default() -> Self {
        Self::new()
    }
}
