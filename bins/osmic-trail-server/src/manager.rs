//! Region Manager — downloads PBFs, creates feature packs, manages loaded regions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{Notify, RwLock};
use tracing::{info, warn};

use osmic_compact::feature_pack::FeaturePack;
use osmic_compact::AreaBuilder;
use osmic_core::BBox;
use osmic_index::feature_index::FeatureIndex;
use osmic_osm::feature::Feature;
use osmic_osm::tags::TagStore;

use crate::region::{GeoRegion, RegionRegistry};

/// Loaded region data — features + spatial index + tag store.
pub struct RegionData {
    pub features: Vec<Feature>,
    pub tag_store: Arc<TagStore>,
    pub feature_index: FeatureIndex,
    pub region_id: String,
}

impl RegionData {
    /// Generate a tile blob for a given bbox.
    pub fn generate_tile(
        &self,
        bbox: &BBox,
        hgt_dir: &Path,
    ) -> Vec<u8> {
        let indices = self.feature_index.query_bbox(bbox);

        let hgt_name = crate::hgt_filename(bbox.min_lat, bbox.min_lon);
        let hgt_path = hgt_dir.join(&hgt_name);
        let contour_features = if hgt_path.exists() {
            osmic_compact::contour::generate_contours(&hgt_path, bbox, 20)
                .unwrap_or_default()
        } else {
            vec![]
        };

        let builder = AreaBuilder {
            display_width: 176,
            display_height: 176,
            contour_interval: 20,
        };

        builder.build_subset(
            &self.features,
            &indices,
            bbox,
            &self.tag_store,
            &contour_features,
        )
    }
}

/// Manages multiple regions with on-demand loading and LRU eviction.
pub struct RegionManager {
    registry: RegionRegistry,
    data_dir: PathBuf,
    max_node_id: i64,
    /// Currently loaded regions (keyed by region ID).
    loaded: RwLock<HashMap<String, Arc<RegionData>>>,
    /// Dedup concurrent loads for the same region.
    loading: RwLock<HashMap<String, Arc<Notify>>>,
    /// Ordered list of region IDs by last access (most recent last) for LRU.
    lru: RwLock<Vec<String>>,
    /// Max regions to keep in memory.
    max_loaded: usize,
}

impl RegionManager {
    pub fn new(
        registry: RegionRegistry,
        data_dir: PathBuf,
        max_node_id: i64,
        max_loaded: usize,
    ) -> Self {
        Self {
            registry,
            data_dir,
            max_node_id,
            loaded: RwLock::new(HashMap::new()),
            loading: RwLock::new(HashMap::new()),
            lru: RwLock::new(Vec::new()),
            max_loaded,
        }
    }

    /// Get the region data for a GPS coordinate. Downloads + processes if needed.
    pub async fn get_region(&self, lat: f64, lon: f64) -> Option<Arc<RegionData>> {
        let region = self.registry.find_region(lat, lon)?;
        let region_id = region.id.clone();

        // Check if already loaded
        {
            let loaded = self.loaded.read().await;
            if let Some(data) = loaded.get(&region_id) {
                self.touch_lru(&region_id).await;
                return Some(data.clone());
            }
        }

        // Check if another task is already loading this region
        {
            let loading = self.loading.read().await;
            if let Some(notify) = loading.get(&region_id) {
                let n = notify.clone();
                drop(loading);
                n.notified().await;
                let loaded = self.loaded.read().await;
                return loaded.get(&region_id).cloned();
            }
        }

        // We need to load it — register as loading
        let notify = Arc::new(Notify::new());
        {
            let mut loading = self.loading.write().await;
            loading.insert(region_id.clone(), notify.clone());
        }

        // Load in a blocking task (PBF processing is CPU-intensive)
        let region_clone = region.clone();
        let data_dir = self.data_dir.clone();
        let max_node_id = self.max_node_id;

        let result = tokio::task::spawn_blocking(move || {
            load_region(&region_clone, &data_dir, max_node_id)
        })
        .await
        .ok()
        .flatten();

        // Remove loading flag
        {
            let mut loading = self.loading.write().await;
            loading.remove(&region_id);
        }

        if let Some(data) = result {
            let data = Arc::new(data);

            // Evict if at capacity
            self.evict_if_needed().await;

            // Store
            {
                let mut loaded = self.loaded.write().await;
                loaded.insert(region_id.clone(), data.clone());
            }
            self.touch_lru(&region_id).await;

            // Notify waiters
            notify.notify_waiters();

            Some(data)
        } else {
            notify.notify_waiters();
            None
        }
    }

    /// Find which region covers a point (for cache key namespacing).
    pub fn find_region_id(&self, lat: f64, lon: f64) -> Option<String> {
        self.registry.find_region(lat, lon).map(|r| r.id.clone())
    }

    async fn touch_lru(&self, region_id: &str) {
        let mut lru = self.lru.write().await;
        lru.retain(|id| id != region_id);
        lru.push(region_id.to_string());
    }

    async fn evict_if_needed(&self) {
        let mut lru = self.lru.write().await;
        let mut loaded = self.loaded.write().await;

        while loaded.len() >= self.max_loaded && !lru.is_empty() {
            let evict_id = lru.remove(0);
            if loaded.remove(&evict_id).is_some() {
                info!(region = %evict_id, "evicted region (LRU)");
            }
        }
    }
}

/// Load a region from feature pack (or download PBF + create feature pack).
fn load_region(
    region: &GeoRegion,
    data_dir: &Path,
    max_node_id: i64,
) -> Option<RegionData> {
    let safe_id = region.id.replace('/', "_");
    let fpack_path = data_dir.join(format!("regions/{safe_id}.fpack"));
    let pbf_path = data_dir.join(format!("regions/{safe_id}.osm.pbf"));

    // Try loading existing feature pack
    if fpack_path.exists() {
        info!(region = %region.id, "loading feature pack from disk");
        let start = Instant::now();
        if let Ok(pack) = FeaturePack::read_from(&fpack_path) {
            let (features, tag_store) = pack.into_features();
            let feature_index = FeatureIndex::build(&features);
            info!(
                region = %region.id,
                features = features.len(),
                elapsed_ms = start.elapsed().as_millis(),
                "region loaded from feature pack"
            );
            return Some(RegionData {
                features,
                tag_store,
                feature_index,
                region_id: region.id.clone(),
            });
        }
        warn!(region = %region.id, "failed to load feature pack, re-downloading");
    }

    // Download PBF if not present
    if !pbf_path.exists() {
        info!(region = %region.id, url = %region.pbf_url, "downloading PBF");
        let start = Instant::now();
        std::fs::create_dir_all(pbf_path.parent().unwrap()).ok();

        match reqwest::blocking::get(&region.pbf_url) {
            Ok(resp) => {
                if !resp.status().is_success() {
                    warn!(region = %region.id, status = %resp.status(), "PBF download failed");
                    return None;
                }
                match resp.bytes() {
                    Ok(bytes) => {
                        std::fs::write(&pbf_path, &bytes).ok();
                        info!(
                            region = %region.id,
                            size_mb = bytes.len() / 1024 / 1024,
                            elapsed_s = start.elapsed().as_secs(),
                            "PBF downloaded"
                        );
                    }
                    Err(e) => {
                        warn!(region = %region.id, error = %e, "PBF download read failed");
                        return None;
                    }
                }
            }
            Err(e) => {
                warn!(region = %region.id, error = %e, "PBF download failed");
                return None;
            }
        }
    }

    // Process PBF → feature pack
    info!(region = %region.id, "processing PBF");
    let start = Instant::now();

    use osmic_index::RamNodeLocationStore;
    use osmic_osm::pipeline::PbfProcessor;
    use osmic_osm::LayerSet;

    let node_store = match RamNodeLocationStore::create(max_node_id) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "failed to create node store");
            return None;
        }
    };
    let processor = PbfProcessor::new();
    let result = match processor.process(&pbf_path, &node_store, &LayerSet::all()) {
        Ok(r) => r,
        Err(e) => {
            warn!(region = %region.id, error = %e, "PBF processing failed");
            return None;
        }
    };

    info!(
        region = %region.id,
        features = result.features.len(),
        elapsed_s = start.elapsed().as_secs_f64(),
        "PBF processed"
    );

    // Save feature pack
    let mut bbox = BBox::empty();
    for f in &result.features {
        let fb = f.bbox();
        bbox.expand(fb.min_lon, fb.min_lat);
        bbox.expand(fb.max_lon, fb.max_lat);
    }

    let pack = FeaturePack::from_processed(&result.features, &result.tag_store, &bbox);
    std::fs::create_dir_all(fpack_path.parent().unwrap()).ok();
    if let Err(e) = pack.write_to(&fpack_path) {
        warn!(error = %e, "failed to save feature pack");
    } else {
        info!(region = %region.id, "feature pack saved");
        // Delete PBF to save disk space
        let _ = std::fs::remove_file(&pbf_path);
    }

    let feature_index = FeatureIndex::build(&result.features);
    Some(RegionData {
        features: result.features,
        tag_store: result.tag_store,
        feature_index,
        region_id: region.id.clone(),
    })
}
