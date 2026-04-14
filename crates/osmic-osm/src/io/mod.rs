//! Streaming I/O utilities for PBF processing.
//!
//! This module provides libosmium-inspired producer/consumer infrastructure
//! for PBF blob streaming. The [`blob_stream`] module exposes a bounded
//! crossbeam channel between a dedicated I/O thread and a user-supplied
//! decode pool — this gives explicit memory bounds and backpressure that
//! rayon's `par_bridge` can't guarantee as cleanly.
//!
//! The high-level [`crate::pipeline::PbfProcessor`] uses `osmpbf`'s
//! built-in `par_map_reduce` which is a different (rayon work-stealing)
//! parallelism model. This module is available as an alternative building
//! block for custom pipelines that need explicit I/O decoupling.

#![cfg(feature = "native")]

pub mod blob_stream;

pub use blob_stream::{BlobStream, BlobStreamConfig};
