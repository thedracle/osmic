use osmic_core::Color;
use osmic_osm::feature::{
    FeatureKind, HighwayKind, LanduseKind, LeisureKind, NaturalKind, RailwayKind, WaterKind,
};

use crate::scene::{LineCap, LineJoin};

/// Resolve fill color for a feature kind.
pub fn fill_color(kind: &FeatureKind) -> Option<Color> {
    match kind {
        FeatureKind::Building(_) => Color::from_hex("#dfdbd7"),
        FeatureKind::Landuse(k) => match k {
            LanduseKind::Forest => Color::from_hex("#add19e"),
            LanduseKind::Grass | LanduseKind::Meadow => Color::from_hex("#cdebb0"),
            LanduseKind::Farmland => Color::from_hex("#d5e29e"),
            LanduseKind::Residential => Color::from_hex("#e0d6d0"),
            LanduseKind::Commercial => Color::from_hex("#f2dad9"),
            LanduseKind::Industrial => Color::from_hex("#ebdbe8"),
            LanduseKind::Cemetery => Color::from_hex("#aacbaf"),
            _ => Color::from_hex("#d5cfc8"),
        },
        FeatureKind::Natural(k) => match k {
            NaturalKind::Wood => Color::from_hex("#add19e"),
            NaturalKind::Scrub => Color::from_hex("#c8d7ab"),
            NaturalKind::Grassland => Color::from_hex("#cdebb0"),
            NaturalKind::Sand | NaturalKind::Beach => Color::from_hex("#f5e9c6"),
            NaturalKind::Glacier => Color::from_hex("#ddecec"),
            NaturalKind::Water => Color::from_hex("#aad3df"),
            _ => None,
        },
        FeatureKind::Water(
            WaterKind::Lake | WaterKind::Pond | WaterKind::Reservoir | WaterKind::Basin,
        ) => Color::from_hex("#aad3df"),
        FeatureKind::Leisure(k) => match k {
            LeisureKind::Park | LeisureKind::Garden | LeisureKind::NatureReserve => {
                Color::from_hex("#c8facc")
            }
            LeisureKind::GolfCourse => Color::from_hex("#b5e3b5"),
            _ => None,
        },
        FeatureKind::Amenity(osmic_osm::feature::AmenityKind::Parking) => {
            Color::from_hex("#eeeeee")
        }
        _ => None,
    }
}

/// Resolve stroke color and width for a feature kind.
pub fn stroke_style(kind: &FeatureKind) -> Option<(Color, f32, LineCap, LineJoin)> {
    match kind {
        FeatureKind::Highway(k) => {
            let (color, width) = match k {
                HighwayKind::Motorway | HighwayKind::MotorwayLink => ("#e892a2", 4.0),
                HighwayKind::Trunk | HighwayKind::TrunkLink => ("#f9b29c", 3.5),
                HighwayKind::Primary | HighwayKind::PrimaryLink => ("#fcd6a4", 3.0),
                HighwayKind::Secondary | HighwayKind::SecondaryLink => ("#f7fabf", 2.5),
                HighwayKind::Tertiary | HighwayKind::TertiaryLink => ("#ffffff", 2.0),
                HighwayKind::Residential | HighwayKind::Unclassified => ("#ffffff", 1.5),
                HighwayKind::Service => ("#ffffff", 1.0),
                _ => ("#cccccc", 0.75),
            };
            Some((
                Color::from_hex(color).unwrap(),
                width,
                LineCap::Round,
                LineJoin::Round,
            ))
        }
        FeatureKind::Railway(k) => {
            let width = match k {
                RailwayKind::Rail => 1.5,
                _ => 1.0,
            };
            Some((
                Color::from_hex("#bfbfbf").unwrap(),
                width,
                LineCap::Butt,
                LineJoin::Miter,
            ))
        }
        FeatureKind::Water(k) => match k {
            WaterKind::River => Some((
                Color::from_hex("#aad3df").unwrap(),
                2.5,
                LineCap::Round,
                LineJoin::Round,
            )),
            WaterKind::Stream | WaterKind::Canal => Some((
                Color::from_hex("#aad3df").unwrap(),
                1.5,
                LineCap::Round,
                LineJoin::Round,
            )),
            _ => None,
        },
        FeatureKind::Boundary(_) => Some((
            Color::from_hex("#9e9cab").unwrap(),
            1.5,
            LineCap::Butt,
            LineJoin::Miter,
        )),
        _ => None,
    }
}

/// Z-order for rendering layers (lower = drawn first).
pub fn z_order(kind: &FeatureKind) -> i32 {
    match kind {
        FeatureKind::Landuse(_) => 10,
        FeatureKind::Natural(_) => 20,
        FeatureKind::Leisure(_) => 30,
        FeatureKind::Water(WaterKind::Lake | WaterKind::Pond | WaterKind::Reservoir) => 40,
        FeatureKind::Building(_) => 50,
        FeatureKind::Boundary(_) => 60,
        FeatureKind::Railway(_) => 70,
        FeatureKind::Water(_) => 80,
        FeatureKind::Highway(_) => 100,
        FeatureKind::Amenity(_) => 110,
        FeatureKind::Shop(_) => 115,
        FeatureKind::Tourism(_) => 120,
        FeatureKind::Office(_) => 125,
        FeatureKind::Healthcare(_) => 130,
        FeatureKind::Craft(_) => 135,
        FeatureKind::Historic(_) => 140,
        FeatureKind::Club(_) => 115,
        FeatureKind::Emergency(_) => 112,
        FeatureKind::Education(_) => 113,
        FeatureKind::Place(_) => 200,
    }
}
