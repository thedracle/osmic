//! Tag-based filtering for OSM entity extraction.
//!
//! Supports AND/OR composition and wildcard values.
//!
//! # Examples
//!
//! ```
//! use omm_extract::TagFilter;
//!
//! // Match any entity with office=property_management
//! let filter = TagFilter::tag("office", "property_management");
//!
//! // Match office=property_management OR office=estate_agent
//! let filter = TagFilter::any(vec![
//!     TagFilter::tag("office", "property_management"),
//!     TagFilter::tag("office", "estate_agent"),
//! ]);
//!
//! // Match entities that have a "name" tag (any value)
//! let filter = TagFilter::key_exists("name");
//! ```

use serde::{Deserialize, Serialize};

/// A composable tag filter for matching OSM elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TagFilter {
    /// Match a specific key=value pair.
    Tag { key: String, value: String },
    /// Match if a key exists (any value).
    KeyExists { key: String },
    /// All sub-filters must match (AND).
    All(Vec<TagFilter>),
    /// At least one sub-filter must match (OR).
    Any(Vec<TagFilter>),
    /// Invert the match.
    Not(Box<TagFilter>),
}

impl TagFilter {
    /// Match a specific key=value pair.
    pub fn tag(key: &str, value: &str) -> Self {
        TagFilter::Tag {
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    /// Match if a key exists with any value.
    pub fn key_exists(key: &str) -> Self {
        TagFilter::KeyExists {
            key: key.to_string(),
        }
    }

    /// All sub-filters must match.
    pub fn all(filters: Vec<TagFilter>) -> Self {
        TagFilter::All(filters)
    }

    /// At least one sub-filter must match.
    pub fn any(filters: Vec<TagFilter>) -> Self {
        TagFilter::Any(filters)
    }

    /// Invert this filter.
    pub fn not(filter: TagFilter) -> Self {
        TagFilter::Not(Box::new(filter))
    }

    /// Test whether a set of tags matches this filter.
    pub fn matches(&self, tags: &[(String, String)]) -> bool {
        match self {
            TagFilter::Tag { key, value } => {
                tags.iter().any(|(k, v)| k == key && v == value)
            }
            TagFilter::KeyExists { key } => {
                tags.iter().any(|(k, _)| k == key)
            }
            TagFilter::All(filters) => filters.iter().all(|f| f.matches(tags)),
            TagFilter::Any(filters) => filters.iter().any(|f| f.matches(tags)),
            TagFilter::Not(filter) => !filter.matches(tags),
        }
    }

    /// Parse an OSM-style tag filter string like "office=property_management".
    ///
    /// Supports:
    /// - `key=value` — exact match
    /// - `key=*` — key exists
    /// - Multiple filters separated by spaces → OR
    pub fn parse(input: &str) -> Self {
        let filters: Vec<TagFilter> = input
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|token| {
                if let Some((key, value)) = token.split_once('=') {
                    if value == "*" {
                        TagFilter::key_exists(key)
                    } else {
                        TagFilter::tag(key, value)
                    }
                } else {
                    TagFilter::key_exists(token)
                }
            })
            .collect();

        match filters.len() {
            0 => TagFilter::All(vec![]), // matches everything
            1 => filters.into_iter().next().unwrap(),
            _ => TagFilter::Any(filters),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn test_exact_match() {
        let filter = TagFilter::tag("office", "property_management");
        assert!(filter.matches(&tags(&[("office", "property_management"), ("name", "Acme")])));
        assert!(!filter.matches(&tags(&[("office", "company"), ("name", "Acme")])));
        assert!(!filter.matches(&tags(&[("name", "Acme")])));
    }

    #[test]
    fn test_key_exists() {
        let filter = TagFilter::key_exists("name");
        assert!(filter.matches(&tags(&[("name", "Acme")])));
        assert!(!filter.matches(&tags(&[("office", "company")])));
    }

    #[test]
    fn test_any() {
        let filter = TagFilter::any(vec![
            TagFilter::tag("office", "property_management"),
            TagFilter::tag("office", "estate_agent"),
        ]);
        assert!(filter.matches(&tags(&[("office", "property_management")])));
        assert!(filter.matches(&tags(&[("office", "estate_agent")])));
        assert!(!filter.matches(&tags(&[("office", "company")])));
    }

    #[test]
    fn test_all() {
        let filter = TagFilter::all(vec![
            TagFilter::key_exists("name"),
            TagFilter::tag("office", "property_management"),
        ]);
        assert!(filter.matches(&tags(&[("name", "Acme"), ("office", "property_management")])));
        assert!(!filter.matches(&tags(&[("office", "property_management")])));
    }

    #[test]
    fn test_not() {
        let filter = TagFilter::all(vec![
            TagFilter::key_exists("name"),
            TagFilter::not(TagFilter::tag("access", "private")),
        ]);
        assert!(filter.matches(&tags(&[("name", "Acme")])));
        assert!(!filter.matches(&tags(&[("name", "Acme"), ("access", "private")])));
    }

    #[test]
    fn test_parse() {
        let filter = TagFilter::parse("office=property_management office=estate_agent");
        assert!(filter.matches(&tags(&[("office", "property_management")])));
        assert!(filter.matches(&tags(&[("office", "estate_agent")])));
        assert!(!filter.matches(&tags(&[("office", "company")])));
    }

    #[test]
    fn test_parse_wildcard() {
        let filter = TagFilter::parse("name=*");
        assert!(filter.matches(&tags(&[("name", "Acme")])));
        assert!(!filter.matches(&tags(&[("office", "company")])));
    }
}
