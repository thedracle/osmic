use std::sync::Arc;

use tracing::info;

use crate::error::AccelResult;

use super::batch::GpuBatch;
use super::context::MetalContext;
use super::flatten::{FlattenedBatch, WorkItem};

/// Output of a GPU clip operation: per-geometry optional (vertex data, vertex count).
type ClipResults = Vec<Option<(Vec<f32>, u32)>>;

/// High-level GPU accelerator for geometry clipping.
///
/// Coordinates must be pre-projected to tile-local space.
/// CPU handles simplification; GPU handles clipping.
pub struct GpuAccelerator {
    ctx: Arc<MetalContext>,
}

impl GpuAccelerator {
    pub fn new() -> AccelResult<Self> {
        let ctx = MetalContext::get()?;
        info!("GPU accelerator initialized");
        Ok(Self { ctx })
    }

    /// Clip a batch of pre-projected geometries synchronously.
    pub fn clip_batch(&self, items: &[WorkItem<'_>]) -> AccelResult<ClipResults> {
        if items.is_empty() {
            return Ok(Vec::new());
        }

        let flat = FlattenedBatch::from_work_items(items);
        if flat.descriptors.is_empty() {
            return Ok(Vec::new());
        }

        let batch = GpuBatch::upload(&self.ctx, &flat)?;
        batch.dispatch_clip(&self.ctx)?;

        let results: ClipResults = (0..flat.descriptors.len())
            .map(|i| batch.read_output(i))
            .collect();

        Ok(results)
    }

    /// Dispatch clip asynchronously — returns a pending batch for later readback.
    pub fn clip_batch_async(&self, items: &[WorkItem<'_>]) -> AccelResult<Option<PendingBatch>> {
        if items.is_empty() {
            return Ok(None);
        }

        let flat = FlattenedBatch::from_work_items(items);
        if flat.descriptors.is_empty() {
            return Ok(None);
        }

        let geom_count = flat.descriptors.len();
        let batch = GpuBatch::upload(&self.ctx, &flat)?;
        let cmd_buffer = batch.dispatch_clip_async(&self.ctx)?;

        Ok(Some(PendingBatch {
            batch,
            command_buffer: cmd_buffer,
            geom_count,
        }))
    }
}

/// A GPU batch that has been dispatched but may not have completed yet.
pub struct PendingBatch {
    pub batch: GpuBatch,
    command_buffer: metal::CommandBuffer,
    pub geom_count: usize,
}

impl PendingBatch {
    /// Wait for GPU completion, then read back results.
    pub fn wait_and_read(self) -> Vec<Option<(Vec<f32>, u32)>> {
        self.command_buffer.wait_until_completed();
        (0..self.geom_count)
            .map(|i| self.batch.read_output(i))
            .collect()
    }
}
