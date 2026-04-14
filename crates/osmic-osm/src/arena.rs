//! Arena allocation support for feature storage.
//!
//! # Current state (Phase C2)
//!
//! This module provides:
//!
//! 1. A [`ProcessedArena`] type that wraps a [`bumpalo::Bump`] and can
//!    produce pre-sized feature vectors. The Bump is available for
//!    pipeline code that wants to allocate auxiliary data in an arena.
//! 2. Capacity estimation helpers ([`estimate_feature_count`],
//!    [`estimate_feature_bytes`]) that pre-size `Vec<Feature>` based on
//!    observed feature density in OSM PBF files.
//!
//! # Why not full arena allocation of [`crate::feature::Feature`]?
//!
//! Moving `Vec<Feature>` into a `bumpalo::collections::Vec<Feature>`
//! requires either:
//!
//! - A pervasive lifetime parameter on `ProcessedData` (which infects
//!   every consumer: CLI, tile generator, extractor, examples), or
//! - A self-referential struct (via `ouroboros` or `self_cell`) that
//!   hides the lifetime but adds a runtime layer.
//!
//! For the current shape of `Feature` — which contains `Geometry`
//! (heap-allocated via `geo-types`) and `Tags` (heap-allocated above 4
//! tags via `SmallVec`) — the top-level `Vec` is not the dominant
//! allocation. Pre-sizing the `Vec` via [`estimate_feature_count`]
//! captures most of the realloc-avoidance win without the API surface
//! disruption.
//!
//! Full arena allocation — including `Geometry` and `Tags` payloads
//! stored as packed bytes inside the arena — is tracked in
//! [`FUTURE_WORK.md`](../../../../FUTURE_WORK.md) under item 1.
//!
//! # Example
//!
//! ```
//! use osmic_osm::arena::{estimate_feature_count, ProcessedArena};
//!
//! // A 100 MB PBF typically contains ~7k-10k features per MB, so
//! // estimate_feature_count conservatively returns ~10k per 1MB.
//! let cap = estimate_feature_count(100 * 1024 * 1024);
//! assert!(cap >= 10_000);
//!
//! // Create an arena that also exposes a pre-sized feature Vec.
//! let arena = ProcessedArena::for_pbf_size(100 * 1024 * 1024);
//! let features_vec = arena.new_feature_vec();
//! assert!(features_vec.capacity() >= 10_000);
//! ```

use crate::feature::Feature;
use bumpalo::Bump;

/// Estimate the number of features a PBF of the given size will produce.
///
/// This is a rough heuristic based on observed planet-scale density:
/// about 1 classified feature per ~10 KB of PBF payload on average for
/// the default layer set. The estimate is intentionally conservative
/// (biased toward over-allocation) to minimize realloc pressure.
pub fn estimate_feature_count(pbf_size_bytes: u64) -> usize {
    // ~10K features per MB of PBF, minimum 1024 so tiny inputs still
    // avoid the initial grow-from-zero.
    let per_mb = 10_000u64;
    let mb = pbf_size_bytes / (1024 * 1024);
    let est = (mb.saturating_mul(per_mb)) as usize;
    est.max(1024)
}

/// Estimate the heap bytes needed for a pre-sized `Vec<Feature>` with
/// the given feature count. Useful for sizing a `bumpalo::Bump`.
///
/// Assumes ~256 bytes per feature including `Geometry` overhead — this
/// is conservative for a mix of points, lines, and polygons.
pub fn estimate_feature_bytes(feature_count: usize) -> usize {
    feature_count.saturating_mul(256)
}

/// A bump-allocated arena for pipeline intermediate data.
///
/// Currently, `ProcessedArena` is primarily used to expose a pre-sized
/// `Vec<Feature>` to the PBF processor. It also carries a `bumpalo::Bump`
/// that pipeline code can use for scratch allocations (e.g. intermediate
/// way-geom buffers) that don't need to outlive the processing call.
pub struct ProcessedArena {
    bump: Bump,
    feature_capacity_hint: usize,
}

impl ProcessedArena {
    /// Create an arena sized for a PBF file of roughly `pbf_size_bytes`.
    pub fn for_pbf_size(pbf_size_bytes: u64) -> Self {
        let feature_count = estimate_feature_count(pbf_size_bytes);
        let bytes = estimate_feature_bytes(feature_count);
        Self {
            bump: Bump::with_capacity(bytes),
            feature_capacity_hint: feature_count,
        }
    }

    /// Create an empty arena with no specific size hint.
    pub fn new() -> Self {
        Self {
            bump: Bump::new(),
            feature_capacity_hint: 1024,
        }
    }

    /// Borrow the underlying bump allocator for ad-hoc arena allocations.
    pub fn bump(&self) -> &Bump {
        &self.bump
    }

    /// The estimated feature count this arena was sized for.
    pub fn feature_capacity_hint(&self) -> usize {
        self.feature_capacity_hint
    }

    /// Create a pre-sized `Vec<Feature>` using the arena's capacity hint.
    ///
    /// This returns a standard `Vec` (not a `bumpalo::collections::Vec`)
    /// so it has no lifetime dependency on the arena — the vector owns
    /// its buffer from the global allocator. The arena's capacity hint
    /// is used only to avoid realloc during growth.
    pub fn new_feature_vec(&self) -> Vec<Feature> {
        Vec::with_capacity(self.feature_capacity_hint)
    }
}

impl Default for ProcessedArena {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_scales_with_pbf_size() {
        let tiny = estimate_feature_count(0);
        let small = estimate_feature_count(1024 * 1024); // 1 MB
        let large = estimate_feature_count(1024 * 1024 * 1024); // 1 GB

        assert!(tiny >= 1024, "tiny input must have minimum capacity");
        assert!(small > tiny, "larger PBF must estimate more features");
        assert!(large > small, "much larger PBF must estimate much more");
    }

    #[test]
    fn processed_arena_exposes_capacity_hint() {
        let arena = ProcessedArena::for_pbf_size(10 * 1024 * 1024);
        let hint = arena.feature_capacity_hint();
        assert!(hint >= 1024);

        let vec = arena.new_feature_vec();
        assert!(vec.capacity() >= hint);
        assert_eq!(vec.len(), 0);
    }

    #[test]
    fn bump_is_usable_for_scratch_allocations() {
        let arena = ProcessedArena::new();
        let bump = arena.bump();

        // Allocate some scratch data in the arena
        let ints = bump.alloc_slice_copy(&[1i64, 2, 3, 4]);
        assert_eq!(ints, &[1, 2, 3, 4]);
    }

    #[test]
    fn default_arena_has_minimum_capacity() {
        let arena = ProcessedArena::default();
        assert_eq!(arena.feature_capacity_hint(), 1024);
    }
}
