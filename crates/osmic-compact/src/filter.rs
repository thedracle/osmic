//! Feature filtering for trail-relevant map data.

use osmic_osm::FeatureKind;
use osmic_osm::feature::*;

use crate::format::{FeatureCategory, PoiType};

/// Returns true if this feature kind is relevant for trail/topo maps.
pub fn is_trail_relevant(kind: &FeatureKind) -> bool {
    matches!(
        kind,
        // Trails, paths, and major roads for navigation context
        FeatureKind::Highway(
            HighwayKind::Path
                | HighwayKind::Footway
                | HighwayKind::Track
                | HighwayKind::Bridleway
                | HighwayKind::Cycleway
                | HighwayKind::Steps
                | HighwayKind::Motorway
                | HighwayKind::Trunk
                | HighwayKind::Primary
                | HighwayKind::Secondary
                | HighwayKind::Tertiary
                | HighwayKind::Residential
                | HighwayKind::Unclassified
                | HighwayKind::Service
        ) | FeatureKind::Water(_)
            | FeatureKind::Natural(_)
            | FeatureKind::Landuse(LanduseKind::Forest | LanduseKind::Meadow | LanduseKind::Grass)
            | FeatureKind::Leisure(LeisureKind::Park | LeisureKind::NatureReserve)
            | FeatureKind::Boundary(BoundaryKind::NationalPark | BoundaryKind::Protected)
            | FeatureKind::Contour(_)
            | FeatureKind::Place(
                PlaceKind::City | PlaceKind::Town | PlaceKind::Village | PlaceKind::Hamlet
            )
            | FeatureKind::Tourism(
                TourismKind::CampSite
                    | TourismKind::Viewpoint
                    | TourismKind::PicnicSite
                    | TourismKind::Information
            )
            | FeatureKind::Amenity(AmenityKind::Parking)
    )
}

/// Returns true if this feature should be encoded as a POI (point) rather than a line/polygon.
pub fn is_poi(kind: &FeatureKind) -> bool {
    matches!(
        kind,
        FeatureKind::Natural(NaturalKind::Peak | NaturalKind::Volcano | NaturalKind::Cliff)
            | FeatureKind::Tourism(
                TourismKind::CampSite
                    | TourismKind::Viewpoint
                    | TourismKind::PicnicSite
                    | TourismKind::Information
            )
            | FeatureKind::Amenity(AmenityKind::Parking)
            | FeatureKind::Place(_)
    )
}

/// Map a FeatureKind to the compact binary category.
pub fn to_category(kind: &FeatureKind) -> FeatureCategory {
    match kind {
        FeatureKind::Highway(h) => match h {
            HighwayKind::Path
            | HighwayKind::Footway
            | HighwayKind::Track
            | HighwayKind::Bridleway
            | HighwayKind::Cycleway
            | HighwayKind::Steps => FeatureCategory::Trail,
            _ => FeatureCategory::Road,
        },
        FeatureKind::Water(w) => {
            if matches!(
                w,
                WaterKind::River
                    | WaterKind::Stream
                    | WaterKind::Canal
                    | WaterKind::Drain
                    | WaterKind::Ditch
                    | WaterKind::Coastline
            ) {
                FeatureCategory::WaterLine
            } else {
                FeatureCategory::WaterArea
            }
        }
        FeatureKind::Contour(elev) => {
            if *elev % 100 == 0 {
                FeatureCategory::ContourMajor
            } else {
                FeatureCategory::ContourMinor
            }
        }
        FeatureKind::Natural(_)
        | FeatureKind::Landuse(_)
        | FeatureKind::Leisure(_) => FeatureCategory::NaturalArea,
        FeatureKind::Boundary(_) => FeatureCategory::Boundary,
        _ => FeatureCategory::Trail, // fallback
    }
}

/// Map a FeatureKind to a subcategory nibble (0-15) for style differentiation.
pub fn to_subcategory(kind: &FeatureKind) -> u8 {
    match kind {
        // Trail subtypes
        FeatureKind::Highway(HighwayKind::Path) => 0,
        FeatureKind::Highway(HighwayKind::Footway) => 1,
        FeatureKind::Highway(HighwayKind::Track) => 2,
        FeatureKind::Highway(HighwayKind::Bridleway) => 3,
        FeatureKind::Highway(HighwayKind::Cycleway) => 4,
        FeatureKind::Highway(HighwayKind::Steps) => 5,
        // Road subtypes
        FeatureKind::Highway(HighwayKind::Motorway) => 0,
        FeatureKind::Highway(HighwayKind::Trunk | HighwayKind::Primary) => 1,
        FeatureKind::Highway(HighwayKind::Secondary | HighwayKind::Tertiary) => 2,
        FeatureKind::Highway(_) => 3, // residential/service/other
        _ => 0,
    }
}

/// Map a FeatureKind to a POI type.
pub fn to_poi_type(kind: &FeatureKind) -> PoiType {
    match kind {
        FeatureKind::Natural(NaturalKind::Peak | NaturalKind::Volcano) => PoiType::Peak,
        FeatureKind::Tourism(TourismKind::CampSite) => PoiType::CampSite,
        FeatureKind::Tourism(TourismKind::Viewpoint) => PoiType::Viewpoint,
        FeatureKind::Tourism(TourismKind::PicnicSite) => PoiType::PicnicSite,
        FeatureKind::Amenity(AmenityKind::Parking) => PoiType::Parking,
        FeatureKind::Place(PlaceKind::Village | PlaceKind::Hamlet) => PoiType::Village,
        FeatureKind::Place(PlaceKind::Town) => PoiType::Town,
        FeatureKind::Place(PlaceKind::City) => PoiType::City,
        _ => PoiType::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trail_paths_are_relevant() {
        assert!(is_trail_relevant(&FeatureKind::Highway(HighwayKind::Path)));
        assert!(is_trail_relevant(&FeatureKind::Highway(HighwayKind::Footway)));
        assert!(is_trail_relevant(&FeatureKind::Highway(HighwayKind::Track)));
    }

    #[test]
    fn buildings_are_not_relevant() {
        assert!(!is_trail_relevant(&FeatureKind::Building(BuildingKind::Yes)));
        assert!(!is_trail_relevant(&FeatureKind::Shop(ShopKind::Supermarket)));
    }

    #[test]
    fn peaks_are_pois() {
        assert!(is_poi(&FeatureKind::Natural(NaturalKind::Peak)));
        assert!(!is_poi(&FeatureKind::Highway(HighwayKind::Path)));
    }

    #[test]
    fn contour_categories() {
        assert_eq!(to_category(&FeatureKind::Contour(200)), FeatureCategory::ContourMajor);
        assert_eq!(to_category(&FeatureKind::Contour(220)), FeatureCategory::ContourMinor);
    }

    #[test]
    fn river_is_water_line() {
        assert_eq!(to_category(&FeatureKind::Water(WaterKind::River)), FeatureCategory::WaterLine);
    }

    #[test]
    fn lake_is_water_area() {
        assert_eq!(to_category(&FeatureKind::Water(WaterKind::Lake)), FeatureCategory::WaterArea);
    }
}
