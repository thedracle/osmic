use osmic_core::Geometry;
use serde::{Deserialize, Serialize};

use crate::tags::Tags;

/// Comprehensive OSM highway classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HighwayKind {
    Motorway,
    MotorwayLink,
    Trunk,
    TrunkLink,
    Primary,
    PrimaryLink,
    Secondary,
    SecondaryLink,
    Tertiary,
    TertiaryLink,
    Residential,
    Unclassified,
    Service,
    LivingStreet,
    Pedestrian,
    Track,
    BusGuideway,
    Footway,
    Bridleway,
    Steps,
    Corridor,
    Path,
    Cycleway,
    Other,
}

impl HighwayKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "motorway" => Self::Motorway,
            "motorway_link" => Self::MotorwayLink,
            "trunk" => Self::Trunk,
            "trunk_link" => Self::TrunkLink,
            "primary" => Self::Primary,
            "primary_link" => Self::PrimaryLink,
            "secondary" => Self::Secondary,
            "secondary_link" => Self::SecondaryLink,
            "tertiary" => Self::Tertiary,
            "tertiary_link" => Self::TertiaryLink,
            "residential" => Self::Residential,
            "unclassified" => Self::Unclassified,
            "service" => Self::Service,
            "living_street" => Self::LivingStreet,
            "pedestrian" => Self::Pedestrian,
            "track" => Self::Track,
            "bus_guideway" => Self::BusGuideway,
            "footway" => Self::Footway,
            "bridleway" => Self::Bridleway,
            "steps" => Self::Steps,
            "corridor" => Self::Corridor,
            "path" => Self::Path,
            "cycleway" => Self::Cycleway,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildingKind {
    Yes,
    House,
    Apartments,
    Commercial,
    Industrial,
    Retail,
    Garage,
    Garages,
    Shed,
    Hut,
    Cabin,
    Church,
    Cathedral,
    Mosque,
    Temple,
    Synagogue,
    Hospital,
    School,
    University,
    Kindergarten,
    Hotel,
    Office,
    Other,
}

impl BuildingKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "yes" => Self::Yes,
            "house" => Self::House,
            "apartments" => Self::Apartments,
            "commercial" => Self::Commercial,
            "industrial" => Self::Industrial,
            "retail" => Self::Retail,
            "garage" => Self::Garage,
            "garages" => Self::Garages,
            "shed" => Self::Shed,
            "hut" => Self::Hut,
            "cabin" => Self::Cabin,
            "church" => Self::Church,
            "cathedral" => Self::Cathedral,
            "mosque" => Self::Mosque,
            "temple" => Self::Temple,
            "synagogue" => Self::Synagogue,
            "hospital" => Self::Hospital,
            "school" => Self::School,
            "university" => Self::University,
            "kindergarten" => Self::Kindergarten,
            "hotel" => Self::Hotel,
            "office" => Self::Office,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WaterKind {
    River,
    Stream,
    Canal,
    Drain,
    Ditch,
    Lake,
    Pond,
    Reservoir,
    Basin,
    Wetland,
    Coastline,
    Other,
}

impl WaterKind {
    pub fn from_waterway_value(val: &str) -> Self {
        match val {
            "river" => Self::River,
            "stream" => Self::Stream,
            "canal" => Self::Canal,
            "drain" => Self::Drain,
            "ditch" => Self::Ditch,
            _ => Self::Other,
        }
    }

    pub fn from_water_value(val: &str) -> Self {
        match val {
            "lake" => Self::Lake,
            "pond" => Self::Pond,
            "reservoir" => Self::Reservoir,
            "basin" => Self::Basin,
            "river" => Self::River,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LanduseKind {
    Residential,
    Commercial,
    Industrial,
    Retail,
    Farmland,
    Forest,
    Grass,
    Meadow,
    Orchard,
    Vineyard,
    Cemetery,
    Military,
    Quarry,
    Recreation,
    Other,
}

impl LanduseKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "residential" => Self::Residential,
            "commercial" => Self::Commercial,
            "industrial" => Self::Industrial,
            "retail" => Self::Retail,
            "farmland" | "farm" | "farmyard" => Self::Farmland,
            "forest" => Self::Forest,
            "grass" => Self::Grass,
            "meadow" => Self::Meadow,
            "orchard" => Self::Orchard,
            "vineyard" => Self::Vineyard,
            "cemetery" => Self::Cemetery,
            "military" => Self::Military,
            "quarry" => Self::Quarry,
            "recreation_ground" => Self::Recreation,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NaturalKind {
    Water,
    Wood,
    Scrub,
    Grassland,
    Heath,
    Sand,
    Bare,
    Wetland,
    Glacier,
    Beach,
    Cliff,
    Peak,
    Volcano,
    Tree,
    Coastline,
    Other,
}

impl NaturalKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "water" => Self::Water,
            "wood" => Self::Wood,
            "scrub" => Self::Scrub,
            "grassland" => Self::Grassland,
            "heath" => Self::Heath,
            "sand" => Self::Sand,
            "bare_rock" => Self::Bare,
            "wetland" => Self::Wetland,
            "glacier" => Self::Glacier,
            "beach" => Self::Beach,
            "cliff" => Self::Cliff,
            "peak" => Self::Peak,
            "volcano" => Self::Volcano,
            "tree" => Self::Tree,
            "coastline" => Self::Coastline,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RailwayKind {
    Rail,
    Subway,
    Tram,
    LightRail,
    Monorail,
    Narrow,
    Preserved,
    Disused,
    Abandoned,
    Platform,
    Station,
    Other,
}

impl RailwayKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "rail" => Self::Rail,
            "subway" => Self::Subway,
            "tram" => Self::Tram,
            "light_rail" => Self::LightRail,
            "monorail" => Self::Monorail,
            "narrow_gauge" => Self::Narrow,
            "preserved" => Self::Preserved,
            "disused" => Self::Disused,
            "abandoned" => Self::Abandoned,
            "platform" => Self::Platform,
            "station" => Self::Station,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AmenityKind {
    Parking,
    School,
    PlaceOfWorship,
    Restaurant,
    Fuel,
    Hospital,
    Pharmacy,
    Bank,
    Cafe,
    FastFood,
    Pub,
    Bar,
    Police,
    FireStation,
    PostOffice,
    Library,
    University,
    Kindergarten,
    Marketplace,
    Other,
}

impl AmenityKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "parking" => Self::Parking,
            "school" => Self::School,
            "place_of_worship" => Self::PlaceOfWorship,
            "restaurant" => Self::Restaurant,
            "fuel" => Self::Fuel,
            "hospital" => Self::Hospital,
            "pharmacy" => Self::Pharmacy,
            "bank" => Self::Bank,
            "cafe" => Self::Cafe,
            "fast_food" => Self::FastFood,
            "pub" => Self::Pub,
            "bar" => Self::Bar,
            "police" => Self::Police,
            "fire_station" => Self::FireStation,
            "post_office" => Self::PostOffice,
            "library" => Self::Library,
            "university" => Self::University,
            "kindergarten" => Self::Kindergarten,
            "marketplace" => Self::Marketplace,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LeisureKind {
    Park,
    Garden,
    Playground,
    GolfCourse,
    SportsCentre,
    SwimmingPool,
    Stadium,
    Pitch,
    NatureReserve,
    Marina,
    Other,
}

impl LeisureKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "park" => Self::Park,
            "garden" => Self::Garden,
            "playground" => Self::Playground,
            "golf_course" => Self::GolfCourse,
            "sports_centre" => Self::SportsCentre,
            "swimming_pool" => Self::SwimmingPool,
            "stadium" => Self::Stadium,
            "pitch" => Self::Pitch,
            "nature_reserve" => Self::NatureReserve,
            "marina" => Self::Marina,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShopKind {
    Supermarket,
    Convenience,
    Clothes,
    Hairdresser,
    CarRepair,
    Bakery,
    Beauty,
    Car,
    MobilePhone,
    Hardware,
    Butcher,
    Alcohol,
    Furniture,
    Electronics,
    DepartmentStore,
    Mall,
    Bicycle,
    Books,
    Jewelry,
    Gift,
    Florist,
    Pet,
    Sports,
    Optician,
    Other,
}

impl ShopKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "supermarket" => Self::Supermarket,
            "convenience" => Self::Convenience,
            "clothes" => Self::Clothes,
            "hairdresser" => Self::Hairdresser,
            "car_repair" => Self::CarRepair,
            "bakery" => Self::Bakery,
            "beauty" => Self::Beauty,
            "car" => Self::Car,
            "mobile_phone" => Self::MobilePhone,
            "hardware" => Self::Hardware,
            "butcher" => Self::Butcher,
            "alcohol" => Self::Alcohol,
            "furniture" => Self::Furniture,
            "electronics" => Self::Electronics,
            "department_store" => Self::DepartmentStore,
            "mall" => Self::Mall,
            "bicycle" => Self::Bicycle,
            "books" => Self::Books,
            "jewelry" => Self::Jewelry,
            "gift" => Self::Gift,
            "florist" => Self::Florist,
            "pet" => Self::Pet,
            "sports" => Self::Sports,
            "optician" => Self::Optician,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TourismKind {
    Hotel,
    Motel,
    Attraction,
    Museum,
    Viewpoint,
    Information,
    GuestHouse,
    CampSite,
    PicnicSite,
    ThemePark,
    Zoo,
    Hostel,
    Artwork,
    Other,
}

impl TourismKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "hotel" => Self::Hotel,
            "motel" => Self::Motel,
            "attraction" => Self::Attraction,
            "museum" => Self::Museum,
            "viewpoint" => Self::Viewpoint,
            "information" => Self::Information,
            "guest_house" => Self::GuestHouse,
            "camp_site" => Self::CampSite,
            "picnic_site" => Self::PicnicSite,
            "theme_park" => Self::ThemePark,
            "zoo" => Self::Zoo,
            "hostel" => Self::Hostel,
            "artwork" => Self::Artwork,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OfficeKind {
    Company,
    Government,
    Insurance,
    Lawyer,
    EstateAgent,
    Financial,
    It,
    Ngo,
    Accountant,
    Architect,
    Other,
}

impl OfficeKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "company" => Self::Company,
            "government" => Self::Government,
            "insurance" => Self::Insurance,
            "lawyer" => Self::Lawyer,
            "estate_agent" => Self::EstateAgent,
            "financial" => Self::Financial,
            "it" => Self::It,
            "ngo" => Self::Ngo,
            "accountant" => Self::Accountant,
            "architect" => Self::Architect,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HealthcareKind {
    Doctor,
    Dentist,
    Clinic,
    Hospital,
    Pharmacy,
    Optometrist,
    Physiotherapist,
    Laboratory,
    Rehabilitation,
    Other,
}

impl HealthcareKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "doctor" => Self::Doctor,
            "dentist" => Self::Dentist,
            "clinic" => Self::Clinic,
            "hospital" => Self::Hospital,
            "pharmacy" => Self::Pharmacy,
            "optometrist" => Self::Optometrist,
            "physiotherapist" => Self::Physiotherapist,
            "laboratory" => Self::Laboratory,
            "rehabilitation" => Self::Rehabilitation,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CraftKind {
    Carpenter,
    Electrician,
    Plumber,
    Painter,
    Brewery,
    Photographer,
    Tailor,
    Hvac,
    Shoemaker,
    Gardener,
    Locksmith,
    Roofer,
    Other,
}

impl CraftKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "carpenter" => Self::Carpenter,
            "electrician" => Self::Electrician,
            "plumber" => Self::Plumber,
            "painter" => Self::Painter,
            "brewery" => Self::Brewery,
            "photographer" => Self::Photographer,
            "tailor" => Self::Tailor,
            "hvac" => Self::Hvac,
            "shoemaker" => Self::Shoemaker,
            "gardener" => Self::Gardener,
            "locksmith" => Self::Locksmith,
            "roofer" => Self::Roofer,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HistoricKind {
    Monument,
    Memorial,
    Castle,
    Ruins,
    ArchaeologicalSite,
    Fort,
    Battlefield,
    Building,
    Other,
}

impl HistoricKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "monument" => Self::Monument,
            "memorial" => Self::Memorial,
            "castle" => Self::Castle,
            "ruins" => Self::Ruins,
            "archaeological_site" => Self::ArchaeologicalSite,
            "fort" => Self::Fort,
            "battlefield" => Self::Battlefield,
            "building" => Self::Building,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClubKind {
    Sport,
    Social,
    Veterans,
    Music,
    Gaming,
    Fishing,
    Other,
}

impl ClubKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "sport" => Self::Sport,
            "social" => Self::Social,
            "veterans" => Self::Veterans,
            "music" => Self::Music,
            "gaming" => Self::Gaming,
            "fishing" => Self::Fishing,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmergencyKind {
    AmbulanceStation,
    FireStation,
    Hospital,
    Phone,
    Defibrillator,
    AssemblyPoint,
    Other,
}

impl EmergencyKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "ambulance_station" => Self::AmbulanceStation,
            "fire_station" | "fire_hydrant" => Self::FireStation,
            "hospital" => Self::Hospital,
            "phone" => Self::Phone,
            "defibrillator" => Self::Defibrillator,
            "assembly_point" => Self::AssemblyPoint,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EducationKind {
    School,
    University,
    College,
    Kindergarten,
    LanguageSchool,
    DrivingSchool,
    MusicSchool,
    Other,
}

impl EducationKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "school" => Self::School,
            "university" => Self::University,
            "college" => Self::College,
            "kindergarten" => Self::Kindergarten,
            "language_school" => Self::LanguageSchool,
            "driving_school" => Self::DrivingSchool,
            "music_school" => Self::MusicSchool,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BoundaryKind {
    Administrative,
    NationalPark,
    Protected,
    Maritime,
    Other,
}

impl BoundaryKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "administrative" => Self::Administrative,
            "national_park" => Self::NationalPark,
            "protected_area" => Self::Protected,
            "maritime" => Self::Maritime,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceKind {
    City,
    Town,
    Village,
    Hamlet,
    Suburb,
    Neighbourhood,
    IsolatedDwelling,
    Other,
}

impl PlaceKind {
    pub fn from_tag_value(val: &str) -> Self {
        match val {
            "city" => Self::City,
            "town" => Self::Town,
            "village" => Self::Village,
            "hamlet" => Self::Hamlet,
            "suburb" => Self::Suburb,
            "neighbourhood" => Self::Neighbourhood,
            "isolated_dwelling" => Self::IsolatedDwelling,
            _ => Self::Other,
        }
    }
}

/// Top-level feature classification from OSM tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FeatureKind {
    Highway(HighwayKind),
    Building(BuildingKind),
    Water(WaterKind),
    Landuse(LanduseKind),
    Natural(NaturalKind),
    Railway(RailwayKind),
    Amenity(AmenityKind),
    Leisure(LeisureKind),
    Shop(ShopKind),
    Tourism(TourismKind),
    Office(OfficeKind),
    Healthcare(HealthcareKind),
    Craft(CraftKind),
    Historic(HistoricKind),
    Club(ClubKind),
    Emergency(EmergencyKind),
    Education(EducationKind),
    Boundary(BoundaryKind),
    Place(PlaceKind),
}

impl FeatureKind {
    /// Returns true if this feature kind is typically rendered as an area (polygon).
    pub fn is_area(&self) -> bool {
        matches!(
            self,
            FeatureKind::Building(_)
                | FeatureKind::Landuse(_)
                | FeatureKind::Leisure(
                    LeisureKind::Park
                        | LeisureKind::Garden
                        | LeisureKind::GolfCourse
                        | LeisureKind::NatureReserve
                        | LeisureKind::Stadium
                        | LeisureKind::Pitch
                )
                | FeatureKind::Natural(
                    NaturalKind::Water
                        | NaturalKind::Wood
                        | NaturalKind::Scrub
                        | NaturalKind::Grassland
                        | NaturalKind::Heath
                        | NaturalKind::Sand
                        | NaturalKind::Bare
                        | NaturalKind::Wetland
                        | NaturalKind::Glacier
                        | NaturalKind::Beach
                )
                | FeatureKind::Water(
                    WaterKind::Lake | WaterKind::Pond | WaterKind::Reservoir | WaterKind::Basin
                )
                | FeatureKind::Amenity(AmenityKind::Parking)
                | FeatureKind::Boundary(_)
                | FeatureKind::Historic(
                    HistoricKind::Castle | HistoricKind::Fort | HistoricKind::Ruins
                )
                | FeatureKind::Tourism(
                    TourismKind::ThemePark | TourismKind::Zoo | TourismKind::CampSite
                )
        )
    }

    /// Returns true if this feature kind is typically rendered as a line.
    pub fn is_line(&self) -> bool {
        matches!(
            self,
            FeatureKind::Highway(_)
                | FeatureKind::Railway(_)
                | FeatureKind::Water(
                    WaterKind::River
                        | WaterKind::Stream
                        | WaterKind::Canal
                        | WaterKind::Drain
                        | WaterKind::Ditch
                )
        )
    }
}

/// A classified OSM feature with geometry and tags.
pub struct Feature {
    pub id: i64,
    pub kind: FeatureKind,
    pub geometry: Geometry,
    pub tags: Tags,
}

impl Feature {
    /// Compute the bounding box of this feature's geometry.
    pub fn bbox(&self) -> osmic_core::BBox {
        self.geometry.bbox()
    }
}

// === String representation methods (inverse of from_tag_value) ===

impl HighwayKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Motorway => "motorway",
            Self::MotorwayLink => "motorway_link",
            Self::Trunk => "trunk",
            Self::TrunkLink => "trunk_link",
            Self::Primary => "primary",
            Self::PrimaryLink => "primary_link",
            Self::Secondary => "secondary",
            Self::SecondaryLink => "secondary_link",
            Self::Tertiary => "tertiary",
            Self::TertiaryLink => "tertiary_link",
            Self::Residential => "residential",
            Self::Unclassified => "unclassified",
            Self::Service => "service",
            Self::LivingStreet => "living_street",
            Self::Pedestrian => "pedestrian",
            Self::Track => "track",
            Self::BusGuideway => "bus_guideway",
            Self::Footway => "footway",
            Self::Bridleway => "bridleway",
            Self::Steps => "steps",
            Self::Corridor => "corridor",
            Self::Path => "path",
            Self::Cycleway => "cycleway",
            Self::Other => "other",
        }
    }
}

impl BuildingKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::House => "house",
            Self::Apartments => "apartments",
            Self::Commercial => "commercial",
            Self::Industrial => "industrial",
            Self::Retail => "retail",
            Self::Garage => "garage",
            Self::Garages => "garages",
            Self::Shed => "shed",
            Self::Hut => "hut",
            Self::Cabin => "cabin",
            Self::Church => "church",
            Self::Cathedral => "cathedral",
            Self::Mosque => "mosque",
            Self::Temple => "temple",
            Self::Synagogue => "synagogue",
            Self::Hospital => "hospital",
            Self::School => "school",
            Self::University => "university",
            Self::Kindergarten => "kindergarten",
            Self::Hotel => "hotel",
            Self::Office => "office",
            Self::Other => "other",
        }
    }
}

impl WaterKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::River => "river",
            Self::Stream => "stream",
            Self::Canal => "canal",
            Self::Drain => "drain",
            Self::Ditch => "ditch",
            Self::Lake => "lake",
            Self::Pond => "pond",
            Self::Reservoir => "reservoir",
            Self::Basin => "basin",
            Self::Wetland => "wetland",
            Self::Coastline => "coastline",
            Self::Other => "other",
        }
    }
}

impl LanduseKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Residential => "residential",
            Self::Commercial => "commercial",
            Self::Industrial => "industrial",
            Self::Retail => "retail",
            Self::Farmland => "farmland",
            Self::Forest => "forest",
            Self::Grass => "grass",
            Self::Meadow => "meadow",
            Self::Orchard => "orchard",
            Self::Vineyard => "vineyard",
            Self::Cemetery => "cemetery",
            Self::Military => "military",
            Self::Quarry => "quarry",
            Self::Recreation => "recreation_ground",
            Self::Other => "other",
        }
    }
}

impl NaturalKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Water => "water",
            Self::Wood => "wood",
            Self::Scrub => "scrub",
            Self::Grassland => "grassland",
            Self::Heath => "heath",
            Self::Sand => "sand",
            Self::Bare => "bare_rock",
            Self::Wetland => "wetland",
            Self::Glacier => "glacier",
            Self::Beach => "beach",
            Self::Cliff => "cliff",
            Self::Peak => "peak",
            Self::Volcano => "volcano",
            Self::Tree => "tree",
            Self::Coastline => "coastline",
            Self::Other => "other",
        }
    }
}

impl RailwayKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rail => "rail",
            Self::Subway => "subway",
            Self::Tram => "tram",
            Self::LightRail => "light_rail",
            Self::Monorail => "monorail",
            Self::Narrow => "narrow_gauge",
            Self::Preserved => "preserved",
            Self::Disused => "disused",
            Self::Abandoned => "abandoned",
            Self::Platform => "platform",
            Self::Station => "station",
            Self::Other => "other",
        }
    }
}

impl AmenityKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Parking => "parking",
            Self::School => "school",
            Self::PlaceOfWorship => "place_of_worship",
            Self::Restaurant => "restaurant",
            Self::Fuel => "fuel",
            Self::Hospital => "hospital",
            Self::Pharmacy => "pharmacy",
            Self::Bank => "bank",
            Self::Cafe => "cafe",
            Self::FastFood => "fast_food",
            Self::Pub => "pub",
            Self::Bar => "bar",
            Self::Police => "police",
            Self::FireStation => "fire_station",
            Self::PostOffice => "post_office",
            Self::Library => "library",
            Self::University => "university",
            Self::Kindergarten => "kindergarten",
            Self::Marketplace => "marketplace",
            Self::Other => "other",
        }
    }
}

impl LeisureKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Park => "park",
            Self::Garden => "garden",
            Self::Playground => "playground",
            Self::GolfCourse => "golf_course",
            Self::SportsCentre => "sports_centre",
            Self::SwimmingPool => "swimming_pool",
            Self::Stadium => "stadium",
            Self::Pitch => "pitch",
            Self::NatureReserve => "nature_reserve",
            Self::Marina => "marina",
            Self::Other => "other",
        }
    }
}

impl ShopKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Supermarket => "supermarket",
            Self::Convenience => "convenience",
            Self::Clothes => "clothes",
            Self::Hairdresser => "hairdresser",
            Self::CarRepair => "car_repair",
            Self::Bakery => "bakery",
            Self::Beauty => "beauty",
            Self::Car => "car",
            Self::MobilePhone => "mobile_phone",
            Self::Hardware => "hardware",
            Self::Butcher => "butcher",
            Self::Alcohol => "alcohol",
            Self::Furniture => "furniture",
            Self::Electronics => "electronics",
            Self::DepartmentStore => "department_store",
            Self::Mall => "mall",
            Self::Bicycle => "bicycle",
            Self::Books => "books",
            Self::Jewelry => "jewelry",
            Self::Gift => "gift",
            Self::Florist => "florist",
            Self::Pet => "pet",
            Self::Sports => "sports",
            Self::Optician => "optician",
            Self::Other => "other",
        }
    }
}

impl TourismKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Hotel => "hotel",
            Self::Motel => "motel",
            Self::Attraction => "attraction",
            Self::Museum => "museum",
            Self::Viewpoint => "viewpoint",
            Self::Information => "information",
            Self::GuestHouse => "guest_house",
            Self::CampSite => "camp_site",
            Self::PicnicSite => "picnic_site",
            Self::ThemePark => "theme_park",
            Self::Zoo => "zoo",
            Self::Hostel => "hostel",
            Self::Artwork => "artwork",
            Self::Other => "other",
        }
    }
}

impl OfficeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Company => "company",
            Self::Government => "government",
            Self::Insurance => "insurance",
            Self::Lawyer => "lawyer",
            Self::EstateAgent => "estate_agent",
            Self::Financial => "financial",
            Self::It => "it",
            Self::Ngo => "ngo",
            Self::Accountant => "accountant",
            Self::Architect => "architect",
            Self::Other => "other",
        }
    }
}

impl HealthcareKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Doctor => "doctor",
            Self::Dentist => "dentist",
            Self::Clinic => "clinic",
            Self::Hospital => "hospital",
            Self::Pharmacy => "pharmacy",
            Self::Optometrist => "optometrist",
            Self::Physiotherapist => "physiotherapist",
            Self::Laboratory => "laboratory",
            Self::Rehabilitation => "rehabilitation",
            Self::Other => "other",
        }
    }
}

impl CraftKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Carpenter => "carpenter",
            Self::Electrician => "electrician",
            Self::Plumber => "plumber",
            Self::Painter => "painter",
            Self::Brewery => "brewery",
            Self::Photographer => "photographer",
            Self::Tailor => "tailor",
            Self::Hvac => "hvac",
            Self::Shoemaker => "shoemaker",
            Self::Gardener => "gardener",
            Self::Locksmith => "locksmith",
            Self::Roofer => "roofer",
            Self::Other => "other",
        }
    }
}

impl HistoricKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Monument => "monument",
            Self::Memorial => "memorial",
            Self::Castle => "castle",
            Self::Ruins => "ruins",
            Self::ArchaeologicalSite => "archaeological_site",
            Self::Fort => "fort",
            Self::Battlefield => "battlefield",
            Self::Building => "building",
            Self::Other => "other",
        }
    }
}

impl ClubKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sport => "sport",
            Self::Social => "social",
            Self::Veterans => "veterans",
            Self::Music => "music",
            Self::Gaming => "gaming",
            Self::Fishing => "fishing",
            Self::Other => "other",
        }
    }
}

impl EmergencyKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AmbulanceStation => "ambulance_station",
            Self::FireStation => "fire_station",
            Self::Hospital => "hospital",
            Self::Phone => "phone",
            Self::Defibrillator => "defibrillator",
            Self::AssemblyPoint => "assembly_point",
            Self::Other => "other",
        }
    }
}

impl EducationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::School => "school",
            Self::University => "university",
            Self::College => "college",
            Self::Kindergarten => "kindergarten",
            Self::LanguageSchool => "language_school",
            Self::DrivingSchool => "driving_school",
            Self::MusicSchool => "music_school",
            Self::Other => "other",
        }
    }
}

impl BoundaryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Administrative => "administrative",
            Self::NationalPark => "national_park",
            Self::Protected => "protected_area",
            Self::Maritime => "maritime",
            Self::Other => "other",
        }
    }
}

impl PlaceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::City => "city",
            Self::Town => "town",
            Self::Village => "village",
            Self::Hamlet => "hamlet",
            Self::Suburb => "suburb",
            Self::Neighbourhood => "neighbourhood",
            Self::IsolatedDwelling => "isolated_dwelling",
            Self::Other => "other",
        }
    }
}

impl FeatureKind {
    /// MVT layer name for this feature kind.
    pub fn layer_name(&self) -> &'static str {
        match self {
            Self::Highway(_) => "highway",
            Self::Building(_) => "building",
            Self::Water(_) => "water",
            Self::Landuse(_) => "landuse",
            Self::Natural(_) => "natural",
            Self::Railway(_) => "railway",
            Self::Amenity(_) => "amenity",
            Self::Leisure(_) => "leisure",
            Self::Shop(_) => "shop",
            Self::Tourism(_) => "tourism",
            Self::Office(_) => "office",
            Self::Healthcare(_) => "healthcare",
            Self::Craft(_) => "craft",
            Self::Historic(_) => "historic",
            Self::Club(_) => "club",
            Self::Emergency(_) => "emergency",
            Self::Education(_) => "education",
            Self::Boundary(_) => "boundary",
            Self::Place(_) => "place",
        }
    }

    /// Subtype class name for MVT tags.
    pub fn class_name(&self) -> &'static str {
        match self {
            Self::Highway(k) => k.as_str(),
            Self::Building(k) => k.as_str(),
            Self::Water(k) => k.as_str(),
            Self::Landuse(k) => k.as_str(),
            Self::Natural(k) => k.as_str(),
            Self::Railway(k) => k.as_str(),
            Self::Amenity(k) => k.as_str(),
            Self::Leisure(k) => k.as_str(),
            Self::Shop(k) => k.as_str(),
            Self::Tourism(k) => k.as_str(),
            Self::Office(k) => k.as_str(),
            Self::Healthcare(k) => k.as_str(),
            Self::Craft(k) => k.as_str(),
            Self::Historic(k) => k.as_str(),
            Self::Club(k) => k.as_str(),
            Self::Emergency(k) => k.as_str(),
            Self::Education(k) => k.as_str(),
            Self::Boundary(k) => k.as_str(),
            Self::Place(k) => k.as_str(),
        }
    }

    /// Minimum zoom level at which this feature should appear in tiles.
    pub fn min_zoom(&self) -> u8 {
        match self {
            Self::Highway(h) => match h {
                HighwayKind::Motorway | HighwayKind::MotorwayLink => 4,
                HighwayKind::Trunk | HighwayKind::TrunkLink => 5,
                HighwayKind::Primary | HighwayKind::PrimaryLink => 7,
                HighwayKind::Secondary | HighwayKind::SecondaryLink => 9,
                HighwayKind::Tertiary | HighwayKind::TertiaryLink => 11,
                HighwayKind::Residential
                | HighwayKind::Unclassified
                | HighwayKind::LivingStreet => 12,
                HighwayKind::Service | HighwayKind::Pedestrian => 13,
                _ => 14,
            },
            Self::Building(_) => 13,
            Self::Water(w) => match w {
                WaterKind::Coastline => 0,
                WaterKind::Lake | WaterKind::Reservoir => 6,
                WaterKind::River => 8,
                WaterKind::Pond | WaterKind::Basin => 10,
                WaterKind::Stream | WaterKind::Canal => 12,
                _ => 13,
            },
            Self::Landuse(_) => 7,
            Self::Natural(n) => match n {
                NaturalKind::Coastline => 0,
                NaturalKind::Water | NaturalKind::Wood | NaturalKind::Glacier => 6,
                _ => 10,
            },
            Self::Railway(r) => match r {
                RailwayKind::Rail => 8,
                RailwayKind::Subway | RailwayKind::LightRail => 10,
                _ => 12,
            },
            Self::Amenity(_) => 13,
            Self::Shop(_) => 14,
            Self::Office(_) => 14,
            Self::Healthcare(_) => 14,
            Self::Craft(_) => 14,
            Self::Tourism(t) => match t {
                TourismKind::ThemePark | TourismKind::Zoo => 10,
                TourismKind::Hotel | TourismKind::Museum | TourismKind::Attraction => 13,
                _ => 14,
            },
            Self::Historic(h) => match h {
                HistoricKind::Castle | HistoricKind::Fort => 10,
                _ => 13,
            },
            Self::Club(_) => 14,
            Self::Emergency(_) => 13,
            Self::Education(_) => 13,
            Self::Leisure(l) => match l {
                LeisureKind::Park | LeisureKind::NatureReserve => 8,
                LeisureKind::GolfCourse | LeisureKind::Stadium => 10,
                _ => 12,
            },
            Self::Boundary(_) => 2,
            Self::Place(p) => match p {
                PlaceKind::City => 4,
                PlaceKind::Town => 7,
                PlaceKind::Village => 10,
                _ => 12,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_area / is_line mutual exclusion ---

    /// Every FeatureKind variant used in rendering must be either area, line,
    /// or neither — never both simultaneously.
    #[test]
    fn is_area_and_is_line_are_mutually_exclusive() {
        let variants: &[FeatureKind] = &[
            FeatureKind::Highway(HighwayKind::Motorway),
            FeatureKind::Highway(HighwayKind::Residential),
            FeatureKind::Building(BuildingKind::Yes),
            FeatureKind::Building(BuildingKind::House),
            FeatureKind::Water(WaterKind::River),
            FeatureKind::Water(WaterKind::Lake),
            FeatureKind::Water(WaterKind::Pond),
            FeatureKind::Water(WaterKind::Stream),
            FeatureKind::Landuse(LanduseKind::Forest),
            FeatureKind::Natural(NaturalKind::Wood),
            FeatureKind::Natural(NaturalKind::Peak),
            FeatureKind::Railway(RailwayKind::Rail),
            FeatureKind::Amenity(AmenityKind::Parking),
            FeatureKind::Amenity(AmenityKind::Restaurant),
            FeatureKind::Leisure(LeisureKind::Park),
            FeatureKind::Leisure(LeisureKind::Playground),
            FeatureKind::Boundary(BoundaryKind::Administrative),
            FeatureKind::Place(PlaceKind::City),
        ];
        for v in variants {
            assert!(
                !(v.is_area() && v.is_line()),
                "{:?} must not be both area and line",
                v
            );
        }
    }

    // --- Highway is always a line ---

    #[test]
    fn highway_is_always_line() {
        let kinds = [
            HighwayKind::Motorway,
            HighwayKind::Trunk,
            HighwayKind::Primary,
            HighwayKind::Secondary,
            HighwayKind::Tertiary,
            HighwayKind::Residential,
            HighwayKind::Footway,
            HighwayKind::Cycleway,
            HighwayKind::Path,
            HighwayKind::Other,
        ];
        for k in kinds {
            let f = FeatureKind::Highway(k);
            assert!(f.is_line(), "Highway({:?}) must be a line", k);
            assert!(!f.is_area(), "Highway({:?}) must not be an area", k);
        }
    }

    // --- Building is always an area ---

    #[test]
    fn building_is_always_area() {
        let kinds = [
            BuildingKind::Yes,
            BuildingKind::House,
            BuildingKind::Apartments,
            BuildingKind::Church,
            BuildingKind::Other,
        ];
        for k in kinds {
            let f = FeatureKind::Building(k);
            assert!(f.is_area(), "Building({:?}) must be an area", k);
            assert!(!f.is_line(), "Building({:?}) must not be a line", k);
        }
    }

    // --- min_zoom ordering: motorway appears before residential ---

    #[test]
    fn motorway_min_zoom_less_than_residential() {
        let motorway = FeatureKind::Highway(HighwayKind::Motorway).min_zoom();
        let residential = FeatureKind::Highway(HighwayKind::Residential).min_zoom();
        assert!(
            motorway < residential,
            "motorway min_zoom ({}) must be less than residential min_zoom ({})",
            motorway,
            residential
        );
    }

    #[test]
    fn motorway_min_zoom_value() {
        assert_eq!(FeatureKind::Highway(HighwayKind::Motorway).min_zoom(), 4);
    }

    #[test]
    fn residential_min_zoom_value() {
        assert_eq!(
            FeatureKind::Highway(HighwayKind::Residential).min_zoom(),
            12
        );
    }

    // --- HighwayKind as_str / from_tag_value round-trip ---

    #[test]
    fn highway_kind_as_str_from_tag_value_roundtrip() {
        let kinds = [
            HighwayKind::Motorway,
            HighwayKind::MotorwayLink,
            HighwayKind::Trunk,
            HighwayKind::TrunkLink,
            HighwayKind::Primary,
            HighwayKind::PrimaryLink,
            HighwayKind::Secondary,
            HighwayKind::SecondaryLink,
            HighwayKind::Tertiary,
            HighwayKind::TertiaryLink,
            HighwayKind::Residential,
            HighwayKind::Unclassified,
            HighwayKind::Service,
            HighwayKind::LivingStreet,
            HighwayKind::Pedestrian,
            HighwayKind::Track,
            HighwayKind::BusGuideway,
            HighwayKind::Footway,
            HighwayKind::Bridleway,
            HighwayKind::Steps,
            HighwayKind::Corridor,
            HighwayKind::Path,
            HighwayKind::Cycleway,
        ];
        for k in kinds {
            let s = k.as_str();
            let back = HighwayKind::from_tag_value(s);
            assert_eq!(
                back, k,
                "HighwayKind::{:?} round-trip failed via {:?}",
                k, s
            );
        }
    }

    // --- BuildingKind as_str / from_tag_value round-trip ---

    #[test]
    fn building_kind_as_str_from_tag_value_roundtrip() {
        let kinds = [
            BuildingKind::Yes,
            BuildingKind::House,
            BuildingKind::Apartments,
            BuildingKind::Commercial,
            BuildingKind::Industrial,
            BuildingKind::Retail,
            BuildingKind::Garage,
            BuildingKind::Garages,
            BuildingKind::Shed,
            BuildingKind::Hut,
            BuildingKind::Cabin,
            BuildingKind::Church,
            BuildingKind::Cathedral,
            BuildingKind::Mosque,
            BuildingKind::Temple,
            BuildingKind::Synagogue,
            BuildingKind::Hospital,
            BuildingKind::School,
            BuildingKind::University,
            BuildingKind::Kindergarten,
            BuildingKind::Hotel,
            BuildingKind::Office,
        ];
        for k in kinds {
            let s = k.as_str();
            let back = BuildingKind::from_tag_value(s);
            assert_eq!(
                back, k,
                "BuildingKind::{:?} round-trip failed via {:?}",
                k, s
            );
        }
    }

    // --- layer_name returns expected strings ---

    #[test]
    fn layer_name_highway() {
        assert_eq!(
            FeatureKind::Highway(HighwayKind::Motorway).layer_name(),
            "highway"
        );
    }

    #[test]
    fn layer_name_building() {
        assert_eq!(
            FeatureKind::Building(BuildingKind::Yes).layer_name(),
            "building"
        );
    }

    #[test]
    fn layer_name_water() {
        assert_eq!(FeatureKind::Water(WaterKind::River).layer_name(), "water");
    }

    #[test]
    fn layer_name_landuse() {
        assert_eq!(
            FeatureKind::Landuse(LanduseKind::Forest).layer_name(),
            "landuse"
        );
    }

    #[test]
    fn layer_name_natural() {
        assert_eq!(
            FeatureKind::Natural(NaturalKind::Wood).layer_name(),
            "natural"
        );
    }

    #[test]
    fn layer_name_railway() {
        assert_eq!(
            FeatureKind::Railway(RailwayKind::Rail).layer_name(),
            "railway"
        );
    }

    #[test]
    fn layer_name_amenity() {
        assert_eq!(
            FeatureKind::Amenity(AmenityKind::Parking).layer_name(),
            "amenity"
        );
    }

    #[test]
    fn layer_name_leisure() {
        assert_eq!(
            FeatureKind::Leisure(LeisureKind::Park).layer_name(),
            "leisure"
        );
    }

    #[test]
    fn layer_name_boundary() {
        assert_eq!(
            FeatureKind::Boundary(BoundaryKind::Administrative).layer_name(),
            "boundary"
        );
    }

    #[test]
    fn layer_name_place() {
        assert_eq!(FeatureKind::Place(PlaceKind::City).layer_name(), "place");
    }
}
