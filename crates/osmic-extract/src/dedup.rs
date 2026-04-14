//! Name + proximity deduplication for extracted entities.
//!
//! Groups entities by normalized name, then uses spatial proximity
//! to collapse duplicates within a configurable radius. O(n * k)
//! where k is the average group size (typically small).

use std::collections::HashMap;

use crate::entity::Entity;

/// Haversine distance between two WGS84 points in meters.
fn haversine_meters(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const R: f64 = 6_371_000.0; // Earth radius in meters
    let phi1 = lat1.to_radians();
    let phi2 = lat2.to_radians();
    let dphi = (lat2 - lat1).to_radians();
    let dlambda = (lon2 - lon1).to_radians();
    let a = (dphi / 2.0).sin().powi(2) + phi1.cos() * phi2.cos() * (dlambda / 2.0).sin().powi(2);
    R * 2.0 * a.sqrt().atan2((1.0 - a).sqrt())
}

/// Richness score for deduplication — prefer entities with more metadata.
fn richness(entity: &Entity) -> usize {
    let mut score = 0;
    if !entity.address.is_empty() {
        score += 3;
    }
    if !entity.phone.is_empty() {
        score += 2;
    }
    if !entity.website.is_empty() {
        score += 2;
    }
    if !entity.operator.is_empty() {
        score += 1;
    }
    score += entity.tags.len();
    score
}

/// Deduplicate entities by name + geographic proximity.
///
/// Groups entities by normalized name, then within each group collapses
/// entities that are within `radius_meters` of each other, keeping
/// the one with the most metadata.
pub fn deduplicate(entities: Vec<Entity>, radius_meters: f64) -> Vec<Entity> {
    if entities.is_empty() {
        return entities;
    }

    // Group by normalized name
    let mut by_name: HashMap<String, Vec<Entity>> = HashMap::new();
    for entity in entities {
        by_name
            .entry(entity.name.to_lowercase())
            .or_default()
            .push(entity);
    }

    let mut unique: Vec<Entity> = Vec::new();
    for (_, group) in by_name {
        unique.extend(dedup_group(group, radius_meters));
    }

    unique
}

/// Deduplicate a group of same-named entities by proximity.
fn dedup_group(group: Vec<Entity>, radius_meters: f64) -> Vec<Entity> {
    let mut kept: Vec<Entity> = Vec::new();

    for candidate in group {
        let merge_idx = find_merge_target(&kept, &candidate, radius_meters);

        match merge_idx {
            Some(i) => {
                if richness(&candidate) > richness(&kept[i]) {
                    kept[i] = candidate;
                }
            }
            None => kept.push(candidate),
        }
    }

    kept
}

/// Find the index of an existing entity to merge with, if any.
fn find_merge_target(kept: &[Entity], candidate: &Entity, radius_meters: f64) -> Option<usize> {
    for (i, existing) in kept.iter().enumerate() {
        match (existing.lat, existing.lon, candidate.lat, candidate.lon) {
            // Both have coordinates — merge only if within radius
            (Some(elat), Some(elon), Some(clat), Some(clon)) => {
                if haversine_meters(elat, elon, clat, clon) < radius_meters {
                    return Some(i);
                }
            }
            // Both lack coordinates — collapse same-name
            (None, _, None, _) | (_, None, _, None)
                if existing.lat.is_none() && candidate.lat.is_none() =>
            {
                return Some(i);
            }
            // Mixed (one has coords, one doesn't) — don't merge,
            // they could be in different cities
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(name: &str, lat: Option<f64>, lon: Option<f64>, phone: &str) -> Entity {
        Entity {
            name: name.to_string(),
            osm_type: "node".to_string(),
            osm_id: 1,
            lat,
            lon,
            address: String::new(),
            phone: phone.to_string(),
            website: String::new(),
            operator: String::new(),
            tags: String::new(),
        }
    }

    #[test]
    fn test_dedup_same_name_same_location() {
        let entities = vec![
            entity("Acme Corp", Some(25.7), Some(-80.2), ""),
            entity("Acme Corp", Some(25.7), Some(-80.2), "555-1234"),
        ];
        let result = deduplicate(entities, 100.0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].phone, "555-1234"); // kept richer one
    }

    #[test]
    fn test_dedup_same_name_different_location() {
        let entities = vec![
            entity("Acme Corp", Some(25.7), Some(-80.2), ""),
            entity("Acme Corp", Some(40.7), Some(-74.0), ""), // NYC vs Miami
        ];
        let result = deduplicate(entities, 100.0);
        assert_eq!(result.len(), 2); // different locations, not deduped
    }

    #[test]
    fn test_dedup_no_coords_collapse() {
        let entities = vec![
            entity("Acme Corp", None, None, ""),
            entity("Acme Corp", None, None, "555-1234"),
        ];
        let result = deduplicate(entities, 100.0);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_dedup_mixed_coords_not_collapsed() {
        // Entity with coords should NOT be collapsed with same-named entity without coords
        // (they could be in different cities)
        let entities = vec![
            entity("Acme Corp", Some(25.7), Some(-80.2), "555-1111"),
            entity("Acme Corp", None, None, "555-2222"),
        ];
        let result = deduplicate(entities, 100.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_different_names_kept() {
        let entities = vec![
            entity("Acme Corp", Some(25.7), Some(-80.2), ""),
            entity("Globex Corp", Some(25.7), Some(-80.2), ""),
        ];
        let result = deduplicate(entities, 100.0);
        assert_eq!(result.len(), 2);
    }
}
