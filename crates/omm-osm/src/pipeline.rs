use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use geo_types::{Coord, LineString, Polygon};
use osmpbf::{Element, ElementReader};
use tracing::info;

use omm_core::bbox::BBox;
use omm_core::error::{OmmError, OmmResult};
use omm_core::geometry::Geometry;

use crate::classify;
use crate::feature::Feature;
use crate::tags::{TagStore, Tags};

/// Statistics from PBF processing.
#[derive(Debug, Clone)]
pub struct PipelineStats {
    pub node_count: u64,
    pub way_count: u64,
    pub relation_count: u64,
    pub feature_count: u64,
    pub pass1_duration: Duration,
    pub pass2_duration: Duration,
    pub total_duration: Duration,
}

/// Result of processing a PBF file.
pub struct ProcessedData {
    pub tag_store: Arc<TagStore>,
    pub features: Vec<Feature>,
    pub bbox: BBox,
    pub stats: PipelineStats,
}

/// Three-pass PBF processor.
///
/// - Pass 1: Node locations → DenseNodeLocationStore (mmap)
/// - Pass 2: Ways → classify by tags → resolve node coords → emit Features
/// - Pass 3: Relations → assemble multipolygons (future)
pub struct PbfProcessor {
    pub tag_store: Arc<TagStore>,
}

/// Trait for node coordinate lookups (abstracts over mmap store).
pub trait NodeLocationStore: Send + Sync {
    fn set(&self, node_id: i64, lon: f64, lat: f64);
    fn get(&self, node_id: i64) -> Option<omm_core::LonLat>;
}

impl PbfProcessor {
    pub fn new() -> Self {
        Self {
            tag_store: Arc::new(TagStore::new()),
        }
    }

    /// Process a PBF file using the provided node location store.
    pub fn process(
        &self,
        pbf_path: &Path,
        node_store: &dyn NodeLocationStore,
    ) -> OmmResult<ProcessedData> {
        let total_start = Instant::now();

        // Pass 1: Node locations
        info!("Pass 1: Reading node locations...");
        let pass1_start = Instant::now();
        let node_count = self.pass1_nodes(pbf_path, node_store)?;
        let pass1_duration = pass1_start.elapsed();
        info!(
            "Pass 1 complete: {} nodes in {:.2}s ({:.0} nodes/s)",
            node_count,
            pass1_duration.as_secs_f64(),
            node_count as f64 / pass1_duration.as_secs_f64()
        );

        // Pass 2: Ways → Features
        info!("Pass 2: Processing ways...");
        let pass2_start = Instant::now();
        let (features, way_count, bbox) = self.pass2_ways(pbf_path, node_store)?;
        let pass2_duration = pass2_start.elapsed();
        info!(
            "Pass 2 complete: {} ways → {} features in {:.2}s",
            way_count,
            features.len(),
            pass2_duration.as_secs_f64()
        );

        // Pass 3: Relations (stub for Phase 1)
        info!("Pass 3: Counting relations...");
        let relation_count = self.pass3_relations(pbf_path)?;
        info!("Pass 3 complete: {} relations (assembly deferred)", relation_count);

        let total_duration = total_start.elapsed();
        let feature_count = features.len() as u64;

        Ok(ProcessedData {
            tag_store: Arc::clone(&self.tag_store),
            features,
            bbox,
            stats: PipelineStats {
                node_count,
                way_count,
                relation_count,
                feature_count,
                pass1_duration,
                pass2_duration,
                total_duration,
            },
        })
    }

    /// Pass 1: Read all nodes and store their locations.
    fn pass1_nodes(&self, path: &Path, node_store: &dyn NodeLocationStore) -> OmmResult<u64> {
        let reader = ElementReader::from_path(path).map_err(|e| OmmError::Pbf(e.to_string()))?;

        reader
            .par_map_reduce(
                |element| match element {
                    Element::Node(node) => {
                        node_store.set(node.id(), node.lon(), node.lat());
                        1u64
                    }
                    Element::DenseNode(node) => {
                        node_store.set(node.id(), node.lon(), node.lat());
                        1u64
                    }
                    _ => 0,
                },
                || 0u64,
                |a, b| a + b,
            )
            .map_err(|e| OmmError::Pbf(e.to_string()))
    }

    /// Pass 2: Process ways into classified features.
    fn pass2_ways(
        &self,
        path: &Path,
        node_store: &dyn NodeLocationStore,
    ) -> OmmResult<(Vec<Feature>, u64, BBox)> {
        let reader = ElementReader::from_path(path).map_err(|e| OmmError::Pbf(e.to_string()))?;
        let tag_store = &self.tag_store;

        let (features, way_count, bbox, unresolved) = reader
            .par_map_reduce(
                |element| {
                    let mut local_features: Vec<Feature> = Vec::new();
                    let mut local_bbox = BBox::empty();
                    let mut local_way_count = 0u64;
                    let mut local_unresolved = 0u64;

                    if let Element::Way(way) = element {
                        local_way_count = 1;

                        // Intern tags
                        let mut tags = Tags::new();
                        for (k, v) in way.tags() {
                            tags.push(tag_store.intern_key(k), tag_store.intern_value(v));
                        }

                        // Classify
                        if let Some(kind) = classify::classify(&tags, tag_store) {
                            // Resolve node coordinates
                            let refs: Vec<i64> = way.refs().collect();
                            let coords: Vec<Coord<f64>> = refs
                                .iter()
                                .filter_map(|&id| {
                                    node_store.get(id).map(|ll| {
                                        local_bbox.expand(ll.lon, ll.lat);
                                        Coord {
                                            x: ll.lon,
                                            y: ll.lat,
                                        }
                                    })
                                })
                                .collect();

                            if coords.len() >= 2 {
                                let is_closed =
                                    refs.len() >= 4 && refs.first() == refs.last();
                                let geometry = if is_closed && kind.is_area() {
                                    Geometry::Polygon(Polygon::new(
                                        LineString::new(coords),
                                        vec![],
                                    ))
                                } else {
                                    Geometry::Line(LineString::new(coords))
                                };

                                local_features.push(Feature {
                                    id: way.id(),
                                    kind,
                                    geometry,
                                    tags,
                                });
                            } else if !refs.is_empty() {
                                local_unresolved += 1;
                            }
                        }
                    }

                    (local_features, local_way_count, local_bbox, local_unresolved)
                },
                || (Vec::new(), 0u64, BBox::empty(), 0u64),
                |(mut fa, ca, mut ba, ua), (fb, cb, bb, ub)| {
                    fa.extend(fb);
                    ba.extend(&bb);
                    (fa, ca + cb, ba, ua + ub)
                },
            )
            .map_err(|e| OmmError::Pbf(e.to_string()))?;

        if unresolved > 0 {
            info!(
                unresolved,
                "Ways skipped (nodes outside extract or exceeding max_node_id)"
            );
        }

        Ok((features, way_count, bbox))
    }

    /// Pass 3: Count relations (full multipolygon assembly deferred to later phase).
    fn pass3_relations(&self, path: &Path) -> OmmResult<u64> {
        let reader = ElementReader::from_path(path).map_err(|e| OmmError::Pbf(e.to_string()))?;

        reader
            .par_map_reduce(
                |element| match element {
                    Element::Relation(_) => 1u64,
                    _ => 0,
                },
                || 0u64,
                |a, b| a + b,
            )
            .map_err(|e| OmmError::Pbf(e.to_string()))
    }
}

impl Default for PbfProcessor {
    fn default() -> Self {
        Self::new()
    }
}
