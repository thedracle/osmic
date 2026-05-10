//! Contour line generation from SRTM elevation data.
//!
//! Reads `.hgt` files (SRTM1/SRTM3), generates isolines at specified intervals,
//! and returns them as `Feature` objects with `FeatureKind::Contour(elevation)`.

use std::path::Path;

use contour::ContourBuilder;
use geo_types::LineString;
use osmic_core::{BBox, Geometry};
use osmic_osm::feature::{Feature, FeatureKind};
use osmic_osm::tags::Tags;
use srtm_reader::Tile;

/// Generate contour line features from an SRTM `.hgt` file.
///
/// `hgt_path` — path to the SRTM `.hgt` file
/// `bbox` — geographic bounding box to extract contours for
/// `interval` — contour interval in meters (e.g., 20)
///
/// Returns contour features clipped to the bounding box.
pub fn generate_contours(
    hgt_path: &Path,
    bbox: &BBox,
    interval: u16,
) -> Result<Vec<Feature>, ContourError> {
    let tile = Tile::from_file(hgt_path).map_err(|e| ContourError::SrtmRead(format!("{e:?}")))?;

    let extent = tile.resolution.extent();
    let tile_lat = tile.latitude as f64;
    let tile_lon = tile.longitude as f64;

    // Compute the sub-grid that covers our bbox within this 1-degree tile.
    // SRTM data is row-major, north-to-south (row 0 = north edge).
    let col_start = ((bbox.min_lon - tile_lon) * (extent - 1) as f64)
        .floor()
        .max(0.0) as usize;
    let col_end = ((bbox.max_lon - tile_lon) * (extent - 1) as f64)
        .ceil()
        .min((extent - 1) as f64) as usize;
    let row_start = (((tile_lat + 1.0) - bbox.max_lat) * (extent - 1) as f64)
        .floor()
        .max(0.0) as usize;
    let row_end = (((tile_lat + 1.0) - bbox.min_lat) * (extent - 1) as f64)
        .ceil()
        .min((extent - 1) as f64) as usize;

    let sub_width = col_end - col_start + 1;
    let sub_height = row_end - row_start + 1;

    if sub_width < 2 || sub_height < 2 {
        return Ok(vec![]);
    }

    // Extract sub-grid as f64 values for the contour builder
    let mut grid: Vec<f64> = Vec::with_capacity(sub_width * sub_height);
    for row in row_start..=row_end {
        for col in col_start..=col_end {
            let idx = row * extent + col;
            let elev = if idx < tile.data.len() {
                tile.data[idx]
            } else {
                0
            };
            // Treat void values as 0
            let elev = if elev == -9999 || elev == i16::MIN {
                0.0
            } else {
                elev as f64
            };
            grid.push(elev);
        }
    }

    // Determine elevation range and thresholds
    let min_elev = grid.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_elev = grid.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let first_contour = ((min_elev / interval as f64).ceil() * interval as f64) as i64;
    let last_contour = ((max_elev / interval as f64).floor() * interval as f64) as i64;

    if first_contour > last_contour {
        return Ok(vec![]);
    }

    let thresholds: Vec<f64> = (first_contour..=last_contour)
        .step_by(interval as usize)
        .map(|e| e as f64)
        .collect();

    if thresholds.is_empty() {
        return Ok(vec![]);
    }

    // Build contour lines using the contour crate
    let builder = ContourBuilder::new(sub_width, sub_height, false);
    let lines = builder
        .lines(&grid, &thresholds)
        .map_err(|e| ContourError::ContourBuild(format!("{e}")))?;

    // Convert grid-local coordinates back to lon/lat
    let lon_step = 1.0 / (extent - 1) as f64;
    let lat_step = 1.0 / (extent - 1) as f64;
    let origin_lon = tile_lon + col_start as f64 * lon_step;
    let origin_lat = tile_lat + 1.0 - row_start as f64 * lat_step;

    let mut features = Vec::new();
    let mut next_id = -1i64; // Use negative IDs to avoid collision with OSM IDs

    for line in lines {
        let elevation = line.threshold() as u16;
        let multi_line = line.geometry();

        for ls in multi_line.iter() {
            let coords: Vec<geo_types::Coord<f64>> = ls
                .coords()
                .map(|c| {
                    // c.x is column index in sub-grid, c.y is row index
                    let lon = origin_lon + c.x * lon_step;
                    let lat = origin_lat - c.y * lat_step;
                    geo_types::Coord { x: lon, y: lat }
                })
                .collect();

            if coords.len() < 2 {
                continue;
            }

            features.push(Feature {
                id: next_id,
                kind: FeatureKind::Contour(elevation),
                geometry: Geometry::Line(LineString::new(coords)),
                tags: Tags::new(),
            });
            next_id -= 1;
        }
    }

    tracing::info!(
        contours = features.len(),
        interval,
        thresholds = thresholds.len(),
        "generated contour lines"
    );

    Ok(features)
}

/// Errors that can occur during contour generation.
#[derive(Debug, thiserror::Error)]
pub enum ContourError {
    #[error("failed to read SRTM file: {0}")]
    SrtmRead(String),
    #[error("contour generation failed: {0}")]
    ContourBuild(String),
}

#[cfg(test)]
mod tests {
    #[test]
    fn contour_thresholds_calculated_correctly() {
        // Test threshold calculation logic
        let interval = 20u16;
        let min_elev = 1850.0f64;
        let max_elev = 2350.0f64;

        let first = ((min_elev / interval as f64).ceil() * interval as f64) as i64;
        let last = ((max_elev / interval as f64).floor() * interval as f64) as i64;

        assert_eq!(first, 1860);
        assert_eq!(last, 2340);

        let thresholds: Vec<i64> = (first..=last).step_by(interval as usize).collect();
        assert_eq!(thresholds.first(), Some(&1860));
        assert_eq!(thresholds.last(), Some(&2340));
        assert_eq!(thresholds.len(), 25); // (2340-1860)/20 + 1 = 25
    }
}
