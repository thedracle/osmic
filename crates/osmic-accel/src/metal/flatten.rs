use std::f64::consts::PI;

use geo_types::Polygon;
use osmic_core::geometry::Geometry;

use super::types::*;

/// Flattened geometry data ready for GPU upload.
/// Coordinates are PRE-PROJECTED to tile-local [0, extent] space.
pub struct FlattenedBatch {
    /// Interleaved (x, y) pairs — already in tile-local projected space.
    pub coords: Vec<f32>,
    /// Per-geometry descriptors.
    pub descriptors: Vec<GpuGeomDescriptor>,
    /// Per-geometry tile clip info.
    pub tile_infos: Vec<GpuTileInfo>,
    /// Total number of coordinate pairs.
    pub total_coords: u32,
    /// Total output capacity (conservative estimate).
    pub total_output_capacity: u32,
}

/// A work item: one (geometry, tile) pair to process on GPU.
pub struct WorkItem<'a> {
    pub geometry: &'a Geometry,
    pub tile_x: u32,
    pub tile_y: u32,
    pub zoom: u8,
    pub extent: u32,
}

/// Project lon/lat to tile-local coordinates [0, extent].
/// Must match TileTransform::lon_lat_to_tile exactly.
fn project_to_tile(lon: f64, lat: f64, n: f64, tx: f64, ty: f64, extent: f64) -> (f32, f32) {
    let lat_clamped = lat.clamp(-85.051_129, 85.051_129);
    let lat_rad = lat_clamped.to_radians();
    let mx = (lon + 180.0) / 360.0;
    let my = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI) / 2.0;
    let x = (mx * n - tx) * extent;
    let y = (my * n - ty) * extent;
    (x as f32, y as f32)
}

impl FlattenedBatch {
    /// Flatten work items into GPU-ready buffers.
    /// Coordinates are projected to tile-local space during flattening (CPU-side).
    pub fn from_work_items(items: &[WorkItem<'_>]) -> Self {
        let mut coords = Vec::new();
        let mut descriptors = Vec::new();
        let mut tile_infos = Vec::new();
        let mut total_output_capacity: u32 = 0;

        for item in items {
            let coord_offset = (coords.len() / 2) as u32;

            let n = (1u64 << item.zoom) as f64;
            let tx = item.tile_x as f64;
            let ty = item.tile_y as f64;
            let extent = item.extent as f64;
            let buf_f: f32 = 0.05;
            let extent_f = item.extent as f32;

            let (geom_type, coord_count) = match item.geometry {
                Geometry::Point(p) => {
                    let (px, py) = project_to_tile(p.x(), p.y(), n, tx, ty, extent);
                    coords.push(px);
                    coords.push(py);
                    (GEOM_TYPE_POINT, 1u32)
                }
                Geometry::Line(ls) => {
                    let count = ls.0.len() as u32;
                    for c in &ls.0 {
                        let (px, py) = project_to_tile(c.x, c.y, n, tx, ty, extent);
                        coords.push(px);
                        coords.push(py);
                    }
                    (GEOM_TYPE_LINE, count)
                }
                Geometry::Polygon(poly) => {
                    let count = flatten_polygon_projected(poly, &mut coords, n, tx, ty, extent);
                    (GEOM_TYPE_POLYGON, count)
                }
                Geometry::MultiPolygon(mp) => {
                    if let Some(poly) = mp.0.first() {
                        let count = flatten_polygon_projected(poly, &mut coords, n, tx, ty, extent);
                        (GEOM_TYPE_POLYGON, count)
                    } else {
                        continue;
                    }
                }
            };

            let output_offset = total_output_capacity;
            let output_cap = coord_count * 2 + 16;
            total_output_capacity += output_cap;

            descriptors.push(GpuGeomDescriptor {
                coord_offset,
                coord_count,
                ring_offset: 0,
                ring_count: 1,
                geom_type,
                output_offset,
                output_capacity: output_cap,
                _pad: 0,
            });

            // Clip bounds in tile-local space: [-buffer, extent + buffer]
            tile_infos.push(GpuTileInfo {
                clip_min_x: -(extent_f * buf_f),
                clip_min_y: -(extent_f * buf_f),
                clip_max_x: extent_f * (1.0 + buf_f),
                clip_max_y: extent_f * (1.0 + buf_f),
                // These are unused now (projection done CPU-side)
                n: n as f32,
                tx: tx as f32,
                ty: ty as f32,
                extent: extent_f,
            });
        }

        let total_coords = (coords.len() / 2) as u32;

        FlattenedBatch {
            coords,
            descriptors,
            tile_infos,
            total_coords,
            total_output_capacity,
        }
    }
}

fn flatten_polygon_projected(
    poly: &Polygon<f64>,
    coords: &mut Vec<f32>,
    n: f64,
    tx: f64,
    ty: f64,
    extent: f64,
) -> u32 {
    let mut total = 0u32;
    // Exterior ring
    for c in poly.exterior().0.iter() {
        let (px, py) = project_to_tile(c.x, c.y, n, tx, ty, extent);
        coords.push(px);
        coords.push(py);
        total += 1;
    }
    // Skip interior rings for now (GPU clip doesn't handle holes yet)
    total
}
