//! Feature Pack — serializable representation of processed PBF data.
//!
//! Converts `Vec<Feature>` + `TagStore` into a format that can be saved to disk
//! and loaded without re-parsing the PBF file. Uses bincode for compact binary
//! serialization. Tags are resolved to `(String, String)` pairs at write time
//! and re-interned into a fresh `TagStore` at read time.

use std::io;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use osmic_core::{BBox, Geometry};
use osmic_osm::feature::{Feature, FeatureKind};
use osmic_osm::tags::{TagStore, Tags};

/// A feature with tags resolved to strings (serializable).
#[derive(Serialize, Deserialize)]
pub struct PackedFeature {
    pub id: i64,
    pub kind: FeatureKind,
    pub geometry: Geometry,
    pub tags: Vec<(String, String)>,
}

/// Serializable container for all processed features from a PBF region.
#[derive(Serialize, Deserialize)]
pub struct FeaturePack {
    pub version: u8,
    pub bbox: BBox,
    pub features: Vec<PackedFeature>,
}

impl FeaturePack {
    const VERSION: u8 = 1;

    /// Create a feature pack from processed PBF data.
    /// Resolves all tag Spur IDs to strings via the TagStore.
    pub fn from_processed(features: &[Feature], tag_store: &TagStore, bbox: &BBox) -> Self {
        let packed: Vec<PackedFeature> = features
            .iter()
            .map(|f| {
                let tags: Vec<(String, String)> = f
                    .tags
                    .iter()
                    .map(|(k, v)| {
                        (
                            tag_store.resolve(*k).to_string(),
                            tag_store.resolve(*v).to_string(),
                        )
                    })
                    .collect();
                PackedFeature {
                    id: f.id,
                    kind: f.kind,
                    geometry: f.geometry.clone(),
                    tags,
                }
            })
            .collect();

        Self {
            version: Self::VERSION,
            bbox: bbox.clone(),
            features: packed,
        }
    }

    /// Write the feature pack to a file (bincode).
    pub fn write_to(&self, path: &Path) -> io::Result<()> {
        let data = bincode::serialize(self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        std::fs::write(path, data)
    }

    /// Read a feature pack from a file.
    pub fn read_from(path: &Path) -> io::Result<Self> {
        let data = std::fs::read(path)?;
        bincode::deserialize(&data)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Convert back to `Vec<Feature>` + `TagStore` for use with AreaBuilder.
    pub fn into_features(self) -> (Vec<Feature>, Arc<TagStore>) {
        let tag_store = Arc::new(TagStore::new());
        let features: Vec<Feature> = self
            .features
            .into_iter()
            .map(|pf| {
                let mut tags = Tags::with_capacity(pf.tags.len());
                for (k, v) in &pf.tags {
                    let key = tag_store.intern_key(k);
                    let val = tag_store.intern_value(v);
                    tags.push(key, val);
                }
                Feature {
                    id: pf.id,
                    kind: pf.kind,
                    geometry: pf.geometry,
                    tags,
                }
            })
            .collect();
        (features, tag_store)
    }
}
