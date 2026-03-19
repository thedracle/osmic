use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use geo_types::{Coord, LineString, MultiPolygon, Polygon};
use osmpbf::{Element, ElementReader, RelMemberType};
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
        let (mut features, way_count, mut bbox) = self.pass2_ways(pbf_path, node_store)?;
        let pass2_duration = pass2_start.elapsed();
        info!(
            "Pass 2 complete: {} ways → {} features in {:.2}s",
            way_count,
            features.len(),
            pass2_duration.as_secs_f64()
        );

        // Pass 3: Multipolygon relation assembly
        info!("Pass 3: Assembling multipolygon relations...");
        let pass3_start = Instant::now();
        let (mp_features, relation_count) =
            self.pass3_relations(pbf_path, node_store, &mut bbox)?;
        let mp_count = mp_features.len();
        features.extend(mp_features);
        info!(
            "Pass 3 complete: {} relations → {} multipolygons in {:.2}s",
            relation_count,
            mp_count,
            pass3_start.elapsed().as_secs_f64()
        );

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

                    match element {
                        Element::Way(way) => {
                            local_way_count = 1;

                            let mut tags = Tags::new();
                            for (k, v) in way.tags() {
                                tags.push(
                                    tag_store.intern_key(k),
                                    tag_store.intern_value(v),
                                );
                            }

                            if let Some(kind) = classify::classify(&tags, tag_store) {
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
                        Element::Node(node) => {
                            process_poi_node(
                                node.id(), node.lon(), node.lat(), node.tags(),
                                tag_store, &mut local_features, &mut local_bbox,
                            );
                        }
                        Element::DenseNode(node) => {
                            process_poi_node(
                                node.id(), node.lon(), node.lat(), node.tags(),
                                tag_store, &mut local_features, &mut local_bbox,
                            );
                        }
                        _ => {}
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

}

/// Extract a tagged node as a POI point feature if it classifies.
fn process_poi_node<'a>(
    id: i64,
    lon: f64,
    lat: f64,
    tags_iter: impl Iterator<Item = (&'a str, &'a str)>,
    tag_store: &TagStore,
    features: &mut Vec<Feature>,
    bbox: &mut BBox,
) {
    let mut tags = Tags::new();
    let mut has_tags = false;
    for (k, v) in tags_iter {
        has_tags = true;
        tags.push(tag_store.intern_key(k), tag_store.intern_value(v));
    }
    if !has_tags {
        return;
    }
    if let Some(kind) = classify::classify(&tags, tag_store) {
        bbox.expand(lon, lat);
        features.push(Feature {
            id,
            kind,
            geometry: Geometry::Point(geo_types::Point::new(lon, lat)),
            tags,
        });
    }
}

struct RelationInfo {
    id: i64,
    tags: Tags,
    outer_way_ids: Vec<i64>,
    inner_way_ids: Vec<i64>,
}

/// Chain ways end-to-end to form closed rings.
///
/// OSM multipolygon outer/inner ways may need to be concatenated
/// (the end node of one way == the start node of the next) to form
/// a complete closed ring.
fn chain_ways(
    way_ids: &[i64],
    way_geoms: &HashMap<i64, Vec<Coord<f64>>>,
) -> Vec<Vec<Coord<f64>>> {
    let mut remaining: Vec<Vec<Coord<f64>>> = way_ids
        .iter()
        .filter_map(|id| way_geoms.get(id).cloned())
        .filter(|c| c.len() >= 2)
        .collect();

    let mut rings = Vec::new();

    while !remaining.is_empty() {
        let mut ring = remaining.swap_remove(0);

        // Try to extend the ring by finding connecting ways
        let mut changed = true;
        while changed {
            changed = false;
            let end = match ring.last() {
                Some(c) => *c,
                None => break,
            };
            let start = ring[0];

            // Check if ring is already closed
            if ring.len() >= 4 && (end.x - start.x).abs() < 1e-8 && (end.y - start.y).abs() < 1e-8
            {
                break;
            }

            for i in 0..remaining.len() {
                let candidate = &remaining[i];
                let c_start = candidate[0];
                let c_end = *candidate.last().unwrap();

                if (end.x - c_start.x).abs() < 1e-8 && (end.y - c_start.y).abs() < 1e-8 {
                    // Append candidate (forward)
                    let mut way = remaining.swap_remove(i);
                    ring.extend(way.drain(1..)); // skip duplicate start point
                    changed = true;
                    break;
                } else if (end.x - c_end.x).abs() < 1e-8 && (end.y - c_end.y).abs() < 1e-8 {
                    // Append candidate (reversed)
                    let mut way = remaining.swap_remove(i);
                    way.reverse();
                    ring.extend(way.drain(1..));
                    changed = true;
                    break;
                }
            }
        }

        if ring.len() >= 3 {
            rings.push(ring);
        }
    }

    rings
}

impl PbfProcessor {
    /// Pass 3: Assemble multipolygon relations.
    ///
    /// Sub-passes:
    /// 3a: Scan relations to find multipolygons and collect member way IDs
    /// 3b: Re-read PBF to build geometries for member ways
    /// 3c: Assemble multipolygons from way geometries
    fn pass3_relations(
        &self,
        path: &Path,
        node_store: &dyn NodeLocationStore,
        bbox: &mut BBox,
    ) -> OmmResult<(Vec<Feature>, u64)> {
        // 3a: Collect multipolygon relation info
        let reader = ElementReader::from_path(path).map_err(|e| OmmError::Pbf(e.to_string()))?;
        let tag_store = &self.tag_store;

        let relations: Vec<RelationInfo> = reader
            .par_map_reduce(
                |element| {
                    let mut local = Vec::new();
                    if let Element::Relation(rel) = element {
                        let mut is_mp = false;
                        let mut tags = Tags::new();
                        for (k, v) in rel.tags() {
                            if k == "type" && v == "multipolygon" {
                                is_mp = true;
                            }
                            tags.push(tag_store.intern_key(k), tag_store.intern_value(v));
                        }
                        if is_mp {
                            let mut outers = Vec::new();
                            let mut inners = Vec::new();
                            for member in rel.members() {
                                if member.member_type == RelMemberType::Way {
                                    match member.role().unwrap_or("") {
                                        "outer" | "" => outers.push(member.member_id),
                                        "inner" => inners.push(member.member_id),
                                        _ => {}
                                    }
                                }
                            }
                            if !outers.is_empty() {
                                local.push(RelationInfo {
                                    id: rel.id(),
                                    tags,
                                    outer_way_ids: outers,
                                    inner_way_ids: inners,
                                });
                            }
                        }
                    }
                    local
                },
                Vec::new,
                |mut a, b| {
                    a.extend(b);
                    a
                },
            )
            .map_err(|e| OmmError::Pbf(e.to_string()))?;

        let relation_count = relations.len() as u64;
        info!(relations = relation_count, "Multipolygon relations found");

        if relations.is_empty() {
            return Ok((Vec::new(), 0));
        }

        // Collect all needed way IDs
        let needed_ways: HashSet<i64> = relations
            .iter()
            .flat_map(|r| r.outer_way_ids.iter().chain(r.inner_way_ids.iter()))
            .copied()
            .collect();
        info!(ways = needed_ways.len(), "Member ways to resolve");

        // 3b: Re-read PBF to get geometries for member ways
        let reader = ElementReader::from_path(path).map_err(|e| OmmError::Pbf(e.to_string()))?;

        let way_geoms: HashMap<i64, Vec<Coord<f64>>> = reader
            .par_map_reduce(
                |element| {
                    let mut local: HashMap<i64, Vec<Coord<f64>>> = HashMap::new();
                    if let Element::Way(way) = element {
                        if needed_ways.contains(&way.id()) {
                            let coords: Vec<Coord<f64>> = way
                                .refs()
                                .filter_map(|id| {
                                    node_store.get(id).map(|ll| Coord {
                                        x: ll.lon,
                                        y: ll.lat,
                                    })
                                })
                                .collect();
                            if coords.len() >= 2 {
                                local.insert(way.id(), coords);
                            }
                        }
                    }
                    local
                },
                HashMap::new,
                |mut a, b| {
                    a.extend(b);
                    a
                },
            )
            .map_err(|e| OmmError::Pbf(e.to_string()))?;

        info!(resolved = way_geoms.len(), "Member way geometries resolved");

        // 3c: Assemble multipolygons
        let mut features = Vec::new();
        for rel in &relations {
            // Assemble outer rings by chaining ways
            let outer_rings = chain_ways(&rel.outer_way_ids, &way_geoms);
            if outer_rings.is_empty() {
                continue;
            }

            let inner_rings = chain_ways(&rel.inner_way_ids, &way_geoms);

            // Classify by tags
            let kind = match classify::classify(&rel.tags, tag_store) {
                Some(k) => k,
                None => continue,
            };

            // Build polygon(s)
            let geometry = if outer_rings.len() == 1 {
                let exterior = LineString::new(outer_rings.into_iter().next().unwrap());
                let holes: Vec<LineString<f64>> = inner_rings
                    .into_iter()
                    .map(LineString::new)
                    .collect();
                for c in exterior.coords() {
                    bbox.expand(c.x, c.y);
                }
                Geometry::Polygon(Polygon::new(exterior, holes))
            } else {
                // Multiple outer rings → MultiPolygon
                // Simple assignment: all inner rings go to the first polygon
                // (proper assignment would check containment)
                let mut polys: Vec<Polygon<f64>> = Vec::new();
                for (i, ring) in outer_rings.into_iter().enumerate() {
                    let exterior = LineString::new(ring);
                    for c in exterior.coords() {
                        bbox.expand(c.x, c.y);
                    }
                    let holes = if i == 0 {
                        inner_rings.iter().map(|r| LineString::new(r.clone())).collect()
                    } else {
                        vec![]
                    };
                    polys.push(Polygon::new(exterior, holes));
                }
                Geometry::MultiPolygon(MultiPolygon::new(polys))
            };

            features.push(Feature {
                id: rel.id,
                kind,
                geometry,
                tags: rel.tags.clone(),
            });
        }

        info!(features = features.len(), "Multipolygon features assembled");
        Ok((features, relation_count))
    }
}

impl Default for PbfProcessor {
    fn default() -> Self {
        Self::new()
    }
}
