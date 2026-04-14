//! Business entity extracted from OSM data.

use serde::{Deserialize, Serialize};

/// A named business entity extracted from OSM data with contact metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Entity name (from `name` tag)
    pub name: String,
    /// OSM element type: "node", "way", or "relation"
    pub osm_type: String,
    /// OSM element ID
    pub osm_id: i64,
    /// Latitude (WGS84), None if coordinates unavailable (some relations)
    pub lat: Option<f64>,
    /// Longitude (WGS84), None if coordinates unavailable
    pub lon: Option<f64>,
    /// Formatted address from addr:* tags
    pub address: String,
    /// Phone number from phone/contact:phone/telephone tags
    pub phone: String,
    /// Website URL from website/contact:website/url tags
    pub website: String,
    /// Operator name from operator tag
    pub operator: String,
    /// Remaining tags as semicolon-separated key=value pairs
    pub tags: String,
}

impl Entity {
    /// Build a formatted address from OSM addr:* tags.
    pub fn build_address(tags: &[(String, String)]) -> String {
        let get = |key: &str| -> &str {
            tags.iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
                .unwrap_or("")
        };

        let mut parts = Vec::new();
        let housenumber = get("addr:housenumber");
        let street = get("addr:street");
        match (housenumber.is_empty(), street.is_empty()) {
            (false, false) => parts.push(format!("{housenumber} {street}")),
            (true, false) => parts.push(street.to_string()),
            _ => {}
        }

        let city = get("addr:city");
        let state = get("addr:state");
        let postcode = get("addr:postcode");
        if !city.is_empty() {
            parts.push(city.to_string());
        }
        if !state.is_empty() {
            parts.push(state.to_string());
        }
        if !postcode.is_empty() {
            parts.push(postcode.to_string());
        }

        parts.join(", ")
    }

    /// Extract phone number from OSM tag conventions.
    pub fn extract_phone(tags: &[(String, String)]) -> String {
        for key in &["phone", "contact:phone", "telephone"] {
            if let Some((_, v)) = tags.iter().find(|(k, _)| k == key) {
                if !v.is_empty() {
                    return v.clone();
                }
            }
        }
        String::new()
    }

    /// Extract website URL from OSM tag conventions.
    pub fn extract_website(tags: &[(String, String)]) -> String {
        for key in &["website", "contact:website", "url"] {
            if let Some((_, v)) = tags.iter().find(|(k, _)| k == key) {
                if !v.is_empty() {
                    return v.clone();
                }
            }
        }
        String::new()
    }

    /// Format remaining tags as semicolon-separated key=value pairs,
    /// excluding address, contact, and metadata fields.
    pub fn format_tags(tags: &[(String, String)]) -> String {
        let skip_keys: &[&str] = &["name", "phone", "telephone", "website", "url", "operator"];
        let skip_prefixes: &[&str] = &["addr:", "contact:"];

        let mut filtered: Vec<(&str, &str)> = tags
            .iter()
            .filter(|(k, _)| {
                !skip_keys.contains(&k.as_str()) && !skip_prefixes.iter().any(|p| k.starts_with(p))
            })
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        filtered.sort_by_key(|(k, _)| *k);
        filtered
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ")
    }
}
