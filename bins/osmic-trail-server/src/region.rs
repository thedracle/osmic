//! Geofabrik region registry — maps GPS coordinates to downloadable PBF extracts.

use std::path::Path;

use serde::Deserialize;
use tracing::info;

/// A Geofabrik region with its download URL and coverage bbox.
#[derive(Debug, Clone)]
pub struct GeoRegion {
    pub id: String,
    pub name: String,
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
    pub pbf_url: String,
    /// Approximate area in square degrees (smaller = more specific region)
    pub area: f64,
}

impl GeoRegion {
    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        lon >= self.min_lon && lon <= self.max_lon && lat >= self.min_lat && lat <= self.max_lat
    }
}

/// Index of all available Geofabrik regions.
pub struct RegionRegistry {
    regions: Vec<GeoRegion>,
}

/// Geofabrik index-v1.json GeoJSON structure.
#[derive(Deserialize)]
struct GeofabrikIndex {
    features: Vec<GeofabrikFeature>,
}

#[derive(Deserialize)]
struct GeofabrikFeature {
    properties: GeofabrikProps,
    #[serde(default)]
    geometry: Option<GeofabrikGeometry>,
}

#[derive(Deserialize)]
struct GeofabrikProps {
    id: String,
    name: String,
    #[serde(default)]
    urls: GeofabrikUrls,
}

#[derive(Deserialize, Default)]
struct GeofabrikUrls {
    #[serde(rename = "pbf", default)]
    pbf: Option<String>,
}

/// GeoJSON geometry — we extract the bbox from the coordinate extents.
#[derive(Deserialize)]
struct GeofabrikGeometry {
    #[serde(default)]
    coordinates: serde_json::Value,
}

impl RegionRegistry {
    /// Load from a cached Geofabrik index file, or download it.
    pub fn load(data_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let index_path = data_dir.join("geofabrik-index.json");

        if !index_path.exists() {
            info!("downloading Geofabrik region index (full, with geometry)");
            let url = "https://download.geofabrik.de/index-v1.json";
            let resp = reqwest::blocking::get(url)?;
            let bytes = resp.bytes()?;
            std::fs::create_dir_all(data_dir)?;
            std::fs::write(&index_path, &bytes)?;
            info!(size_kb = bytes.len() / 1024, "Geofabrik index cached");
        }

        let data = std::fs::read_to_string(&index_path)?;
        let index: GeofabrikIndex = serde_json::from_str(&data)?;

        let mut regions = Vec::new();
        for feat in &index.features {
            let props = &feat.properties;
            let pbf_url = match &props.urls.pbf {
                Some(u) => u.clone(),
                None => continue,
            };

            // Extract bbox from geometry coordinates
            let (min_lon, min_lat, max_lon, max_lat) = match &feat.geometry {
                Some(geom) => extract_bbox_from_geojson(&geom.coordinates),
                None => continue,
            };

            if min_lon >= max_lon || min_lat >= max_lat {
                continue;
            }

            let area = (max_lon - min_lon) * (max_lat - min_lat);
            regions.push(GeoRegion {
                id: props.id.clone(),
                name: props.name.clone(),
                min_lon,
                min_lat,
                max_lon,
                max_lat,
                pbf_url,
                area,
            });
        }

        info!(regions = regions.len(), "Geofabrik region index loaded");
        Ok(Self { regions })
    }

    /// Find the smallest region containing the given point.
    pub fn find_region(&self, lat: f64, lon: f64) -> Option<&GeoRegion> {
        self.regions
            .iter()
            .filter(|r| r.contains(lat, lon))
            .min_by(|a, b| a.area.partial_cmp(&b.area).unwrap_or(std::cmp::Ordering::Equal))
    }
}

/// Walk a GeoJSON coordinates value and extract the bounding box.
fn extract_bbox_from_geojson(coords: &serde_json::Value) -> (f64, f64, f64, f64) {
    let mut min_lon = f64::MAX;
    let mut min_lat = f64::MAX;
    let mut max_lon = f64::MIN;
    let mut max_lat = f64::MIN;

    fn walk(v: &serde_json::Value, min_lon: &mut f64, min_lat: &mut f64, max_lon: &mut f64, max_lat: &mut f64) {
        match v {
            serde_json::Value::Array(arr) => {
                // A coordinate pair is [lon, lat] — both are numbers
                if arr.len() >= 2 {
                    if let (Some(lon), Some(lat)) = (arr[0].as_f64(), arr[1].as_f64()) {
                        if lon.is_finite() && lat.is_finite() {
                            if lon < *min_lon { *min_lon = lon; }
                            if lon > *max_lon { *max_lon = lon; }
                            if lat < *min_lat { *min_lat = lat; }
                            if lat > *max_lat { *max_lat = lat; }
                            return;
                        }
                    }
                }
                // Not a coord pair — recurse into nested arrays
                for item in arr {
                    walk(item, min_lon, min_lat, max_lon, max_lat);
                }
            }
            _ => {}
        }
    }

    walk(coords, &mut min_lon, &mut min_lat, &mut max_lon, &mut max_lat);
    (min_lon, min_lat, max_lon, max_lat)
}
