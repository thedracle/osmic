use lasso::{Spur, ThreadedRodeo};
use smallvec::SmallVec;

/// Interned tag key (compact integer ID).
pub type TagKey = Spur;
/// Interned tag value (compact integer ID).
pub type TagValue = Spur;

/// Well-known OSM tag keys that appear millions of times.
/// Pre-interned for zero-cost matching in hot paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum WellKnownKey {
    Highway = 0,
    Building,
    Name,
    Waterway,
    Natural,
    Landuse,
    Railway,
    Amenity,
    Leisure,
    Boundary,
    Place,
    Shop,
    Tourism,
    Power,
    Aeroway,
    Surface,
    Maxspeed,
    Ref,
    Oneway,
    Bridge,
    Tunnel,
    Layer,
    Access,
    Service,
    Foot,
    Bicycle,
    Lanes,
    Lit,
    AdminLevel,
    Water,
}

impl WellKnownKey {
    pub const ALL: &[WellKnownKey] = &[
        Self::Highway,
        Self::Building,
        Self::Name,
        Self::Waterway,
        Self::Natural,
        Self::Landuse,
        Self::Railway,
        Self::Amenity,
        Self::Leisure,
        Self::Boundary,
        Self::Place,
        Self::Shop,
        Self::Tourism,
        Self::Power,
        Self::Aeroway,
        Self::Surface,
        Self::Maxspeed,
        Self::Ref,
        Self::Oneway,
        Self::Bridge,
        Self::Tunnel,
        Self::Layer,
        Self::Access,
        Self::Service,
        Self::Foot,
        Self::Bicycle,
        Self::Lanes,
        Self::Lit,
        Self::AdminLevel,
        Self::Water,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Highway => "highway",
            Self::Building => "building",
            Self::Name => "name",
            Self::Waterway => "waterway",
            Self::Natural => "natural",
            Self::Landuse => "landuse",
            Self::Railway => "railway",
            Self::Amenity => "amenity",
            Self::Leisure => "leisure",
            Self::Boundary => "boundary",
            Self::Place => "place",
            Self::Shop => "shop",
            Self::Tourism => "tourism",
            Self::Power => "power",
            Self::Aeroway => "aeroway",
            Self::Surface => "surface",
            Self::Maxspeed => "maxspeed",
            Self::Ref => "ref",
            Self::Oneway => "oneway",
            Self::Bridge => "bridge",
            Self::Tunnel => "tunnel",
            Self::Layer => "layer",
            Self::Access => "access",
            Self::Service => "service",
            Self::Foot => "foot",
            Self::Bicycle => "bicycle",
            Self::Lanes => "lanes",
            Self::Lit => "lit",
            Self::AdminLevel => "admin_level",
            Self::Water => "water",
        }
    }
}

/// Shared string interner for tag keys and values.
///
/// Thread-safe: multiple threads can intern simultaneously during parallel PBF parsing.
pub struct TagStore {
    rodeo: ThreadedRodeo,
    well_known: Vec<TagKey>,
}

impl TagStore {
    pub fn new() -> Self {
        let rodeo = ThreadedRodeo::default();
        let well_known: Vec<TagKey> = WellKnownKey::ALL
            .iter()
            .map(|wk| rodeo.get_or_intern(wk.as_str()))
            .collect();
        Self { rodeo, well_known }
    }

    /// Intern a tag key string, returning its compact ID.
    pub fn intern_key(&self, key: &str) -> TagKey {
        self.rodeo.get_or_intern(key)
    }

    /// Intern a tag value string, returning its compact ID.
    pub fn intern_value(&self, value: &str) -> TagValue {
        self.rodeo.get_or_intern(value)
    }

    /// Resolve an interned key/value back to its string.
    pub fn resolve(&self, key: Spur) -> &str {
        self.rodeo.resolve(&key)
    }

    /// Get the pre-interned key for a well-known OSM tag.
    pub fn well_known(&self, wk: WellKnownKey) -> TagKey {
        self.well_known[wk as usize]
    }

    /// Try to look up a string without interning it.
    pub fn get(&self, key: &str) -> Option<Spur> {
        self.rodeo.get(key)
    }

    /// Number of unique strings interned.
    pub fn len(&self) -> usize {
        self.rodeo.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rodeo.is_empty()
    }
}

impl Default for TagStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Compact tag storage for a single OSM element.
///
/// Uses `SmallVec` with inline capacity of 4, since most OSM elements
/// have 0-5 tags. Avoids heap allocation for the common case.
#[derive(Debug, Clone)]
pub struct Tags {
    inner: SmallVec<[(TagKey, TagValue); 4]>,
}

impl Tags {
    pub fn new() -> Self {
        Self {
            inner: SmallVec::new(),
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            inner: SmallVec::with_capacity(cap),
        }
    }

    pub fn push(&mut self, key: TagKey, value: TagValue) {
        self.inner.push((key, value));
    }

    /// Look up a tag value by key. Linear scan (fast for small N).
    pub fn get(&self, key: TagKey) -> Option<TagValue> {
        self.inner
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    }

    pub fn contains(&self, key: TagKey) -> bool {
        self.inner.iter().any(|(k, _)| *k == key)
    }

    pub fn iter(&self) -> impl Iterator<Item = &(TagKey, TagValue)> {
        self.inner.iter()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for Tags {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- TagStore: well_known resolves to the correct string ---

    #[test]
    fn well_known_resolves_to_correct_string() {
        let store = TagStore::new();
        for wk in WellKnownKey::ALL {
            let key = store.well_known(*wk);
            let resolved = store.resolve(key);
            assert_eq!(
                resolved,
                wk.as_str(),
                "WellKnownKey::{:?} resolved to {:?}, expected {:?}",
                wk,
                resolved,
                wk.as_str()
            );
        }
    }

    #[test]
    fn well_known_highway_key_is_interned_before_custom_keys() {
        let store = TagStore::new();
        // Interning "highway" again must return the same Spur as well_known(Highway).
        let via_intern = store.intern_key("highway");
        let via_well_known = store.well_known(WellKnownKey::Highway);
        assert_eq!(via_intern, via_well_known);
    }

    // --- intern_key / resolve round-trip ---

    #[test]
    fn intern_key_resolve_roundtrip() {
        let store = TagStore::new();
        let key = store.intern_key("custom_key");
        assert_eq!(store.resolve(key), "custom_key");
    }

    #[test]
    fn intern_value_resolve_roundtrip() {
        let store = TagStore::new();
        let val = store.intern_value("some_value");
        assert_eq!(store.resolve(val), "some_value");
    }

    #[test]
    fn intern_same_string_twice_returns_same_spur() {
        let store = TagStore::new();
        let a = store.intern_key("duplicate");
        let b = store.intern_key("duplicate");
        assert_eq!(a, b);
    }

    #[test]
    fn get_returns_none_for_missing_key() {
        let store = TagStore::new();
        assert!(store.get("this_key_was_never_interned_xyz").is_none());
    }

    #[test]
    fn get_returns_some_after_interning() {
        let store = TagStore::new();
        store.intern_key("present");
        assert!(store.get("present").is_some());
    }

    // --- Tags: get / contains / len ---

    #[test]
    fn tags_get_and_contains() {
        let store = TagStore::new();
        let highway_key = store.well_known(WellKnownKey::Highway);
        let motorway_val = store.intern_value("motorway");

        let mut tags = Tags::new();
        assert_eq!(tags.len(), 0);
        assert!(tags.is_empty());
        assert!(!tags.contains(highway_key));
        assert!(tags.get(highway_key).is_none());

        tags.push(highway_key, motorway_val);
        assert_eq!(tags.len(), 1);
        assert!(!tags.is_empty());
        assert!(tags.contains(highway_key));
        assert_eq!(tags.get(highway_key), Some(motorway_val));
    }

    #[test]
    fn tags_get_absent_key_returns_none() {
        let store = TagStore::new();
        let building_key = store.well_known(WellKnownKey::Building);
        let name_key = store.well_known(WellKnownKey::Name);
        let yes_val = store.intern_value("yes");

        let mut tags = Tags::new();
        tags.push(building_key, yes_val);

        // name was never pushed.
        assert!(tags.get(name_key).is_none());
        assert!(!tags.contains(name_key));
    }

    // --- Tags: capacity exceeding inline SmallVec ---

    #[test]
    fn tags_exceeds_inline_capacity() {
        let store = TagStore::new();
        // The inline capacity is 4; push more than 4 pairs to force heap allocation.
        let mut tags = Tags::with_capacity(8);
        let keys: Vec<TagKey> = WellKnownKey::ALL
            .iter()
            .take(8)
            .map(|wk| store.well_known(*wk))
            .collect();
        let val = store.intern_value("test");

        for &k in &keys {
            tags.push(k, val);
        }

        assert_eq!(tags.len(), 8);

        // Every pushed key must be retrievable.
        for &k in &keys {
            assert!(tags.contains(k));
            assert_eq!(tags.get(k), Some(val));
        }
    }
}
