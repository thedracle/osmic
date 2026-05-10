/// Bitmask of enabled OSM feature layers for extraction.
///
/// Used by `classify()` to skip disabled layers at parse time.
/// Default is all layers enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerSet(u32);

impl LayerSet {
    pub const HIGHWAY: u32 = 1 << 0;
    pub const BUILDING: u32 = 1 << 1;
    pub const WATER: u32 = 1 << 2;
    pub const NATURAL: u32 = 1 << 3;
    pub const LANDUSE: u32 = 1 << 4;
    pub const RAILWAY: u32 = 1 << 5;
    pub const AMENITY: u32 = 1 << 6;
    pub const LEISURE: u32 = 1 << 7;
    pub const BOUNDARY: u32 = 1 << 8;
    pub const PLACE: u32 = 1 << 9;
    pub const SHOP: u32 = 1 << 10;
    pub const TOURISM: u32 = 1 << 11;
    pub const OFFICE: u32 = 1 << 12;
    pub const HEALTHCARE: u32 = 1 << 13;
    pub const CRAFT: u32 = 1 << 14;
    pub const HISTORIC: u32 = 1 << 15;
    pub const CLUB: u32 = 1 << 16;
    pub const EMERGENCY: u32 = 1 << 17;
    pub const EDUCATION: u32 = 1 << 18;
    pub const CONTOUR: u32 = 1 << 19;

    const NAME_MAP: &[(&'static str, u32)] = &[
        ("highway", Self::HIGHWAY),
        ("building", Self::BUILDING),
        ("water", Self::WATER),
        ("natural", Self::NATURAL),
        ("landuse", Self::LANDUSE),
        ("railway", Self::RAILWAY),
        ("amenity", Self::AMENITY),
        ("leisure", Self::LEISURE),
        ("boundary", Self::BOUNDARY),
        ("place", Self::PLACE),
        ("shop", Self::SHOP),
        ("tourism", Self::TOURISM),
        ("office", Self::OFFICE),
        ("healthcare", Self::HEALTHCARE),
        ("craft", Self::CRAFT),
        ("historic", Self::HISTORIC),
        ("club", Self::CLUB),
        ("emergency", Self::EMERGENCY),
        ("education", Self::EDUCATION),
        ("contour", Self::CONTOUR),
    ];

    /// All layers enabled.
    pub fn all() -> Self {
        Self(0xFFFFF) // 20 bits
    }

    /// No layers enabled.
    pub fn none() -> Self {
        Self(0)
    }

    /// Parse a comma-separated list of layer names.
    ///
    /// Returns `Err` with the first unrecognized name.
    pub fn from_names(input: &str) -> Result<Self, String> {
        let mut bits = 0u32;
        for name in input.split(',') {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            match Self::NAME_MAP.iter().find(|(n, _)| *n == name) {
                Some((_, bit)) => bits |= bit,
                None => return Err(format!("unknown layer: '{name}'")),
            }
        }
        Ok(Self(bits))
    }

    /// Check if a layer is enabled by its layer_name string.
    pub fn is_enabled(&self, layer_name: &str) -> bool {
        match Self::NAME_MAP.iter().find(|(n, _)| *n == layer_name) {
            Some((_, bit)) => self.0 & bit != 0,
            None => false,
        }
    }

    /// Returns all available layer names.
    pub fn available_names() -> impl Iterator<Item = &'static str> {
        Self::NAME_MAP.iter().map(|(name, _)| *name)
    }
}

impl Default for LayerSet {
    fn default() -> Self {
        Self::all()
    }
}

impl std::fmt::Display for LayerSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = Self::NAME_MAP
            .iter()
            .filter(|(_, bit)| self.0 & bit != 0)
            .map(|(name, _)| *name)
            .collect();
        write!(f, "{}", names.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_enables_everything() {
        let set = LayerSet::all();
        for (name, _) in LayerSet::NAME_MAP {
            assert!(set.is_enabled(name), "{name} should be enabled in all()");
        }
    }

    #[test]
    fn none_disables_everything() {
        let set = LayerSet::none();
        for (name, _) in LayerSet::NAME_MAP {
            assert!(!set.is_enabled(name), "{name} should be disabled in none()");
        }
    }

    #[test]
    fn from_names_parses_subset() {
        let set = LayerSet::from_names("amenity,shop,tourism").unwrap();
        assert!(set.is_enabled("amenity"));
        assert!(set.is_enabled("shop"));
        assert!(set.is_enabled("tourism"));
        assert!(!set.is_enabled("highway"));
        assert!(!set.is_enabled("building"));
    }

    #[test]
    fn from_names_rejects_unknown() {
        let err = LayerSet::from_names("amenity,bogus").unwrap_err();
        assert!(err.contains("bogus"));
    }

    #[test]
    fn display_format() {
        let set = LayerSet::from_names("shop,amenity").unwrap();
        let s = set.to_string();
        assert!(s.contains("amenity"));
        assert!(s.contains("shop"));
    }

    #[test]
    fn default_is_all() {
        assert_eq!(LayerSet::default(), LayerSet::all());
    }
}
