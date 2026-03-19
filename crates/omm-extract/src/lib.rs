//! Business entity extraction from OSM PBF data.
//!
//! Unlike `omm-osm` which classifies features for map rendering, `omm-extract`
//! extracts named business entities with contact metadata (phone, website, address)
//! using arbitrary tag-based filtering. Designed for lead generation pipelines.
//!
//! # Pipeline
//!
//! ```text
//! PBF file
//!   → Pass 1: Store node locations (mmap via DenseNodeLocationStore)
//!   → Pass 2: Filter nodes/ways/relations by tag rules, extract entities
//!   → Deduplicate by name + proximity (R-tree backed)
//!   → Output to CSV or JSON
//! ```

pub mod dedup;
pub mod entity;
pub mod filter;
pub mod output;
pub mod pipeline;

pub use dedup::deduplicate;
pub use entity::Entity;
pub use filter::TagFilter;
pub use output::{write_csv, write_json};
pub use pipeline::{ExtractConfig, ExtractResult, Extractor};
