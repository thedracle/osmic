use std::sync::Arc;

use metal::MTLSize;

use crate::error::{AccelError, AccelResult};

use super::buffer::MetalBuffer;
use super::context::MetalContext;
use super::flatten::FlattenedBatch;
use super::types::*;

/// A batch of pre-projected geometries uploaded to GPU for clipping.
pub struct GpuBatch {
    pub coords: MetalBuffer<f32>,
    pub output_coords: MetalBuffer<f32>,
    pub descriptors: MetalBuffer<GpuGeomDescriptor>,
    pub tile_infos: MetalBuffer<GpuTileInfo>,
    pub output_headers: MetalBuffer<GpuOutputHeader>,
    pub geometry_count: u32,
}

impl GpuBatch {
    pub fn upload(ctx: &Arc<MetalContext>, flat: &FlattenedBatch) -> AccelResult<Self> {
        if flat.descriptors.is_empty() {
            return Err(AccelError::BufferCreation("Empty batch".into()));
        }

        let device = ctx.device();

        let coords = MetalBuffer::from_slice(device, &flat.coords)?;
        let output_coords =
            MetalBuffer::<f32>::new(device, flat.total_output_capacity as usize * 2)?;
        let descriptors = MetalBuffer::from_slice(device, &flat.descriptors)?;
        let tile_infos = MetalBuffer::from_slice(device, &flat.tile_infos)?;

        let header_count = flat.descriptors.len();
        let mut output_headers = MetalBuffer::<GpuOutputHeader>::new(device, header_count)?;
        for h in output_headers.as_mut_slice() {
            *h = GpuOutputHeader {
                output_count: 0,
                output_ring_count: 0,
                status: 0,
                _pad: 0,
            };
        }

        Ok(GpuBatch {
            coords,
            output_coords,
            descriptors,
            tile_infos,
            output_headers,
            geometry_count: flat.descriptors.len() as u32,
        })
    }

    /// Dispatch clip kernel synchronously.
    pub fn dispatch_clip(&self, ctx: &Arc<MetalContext>) -> AccelResult<()> {
        let command_buffer = ctx.command_queue().new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        let pipeline = {
            let mut cache = ctx.pipeline_cache();
            cache.get_pipeline(ctx.device(), "batch_clip")?.to_owned()
        };

        encoder.set_compute_pipeline_state(&pipeline);
        encoder.set_buffer(0, Some(self.coords.metal_buffer()), 0);
        encoder.set_buffer(1, Some(self.output_coords.metal_buffer()), 0);
        encoder.set_buffer(2, Some(self.descriptors.metal_buffer()), 0);
        encoder.set_buffer(3, Some(self.tile_infos.metal_buffer()), 0);
        encoder.set_buffer(4, Some(self.output_headers.metal_buffer()), 0);

        let params = GpuClipParams {
            geometry_count: self.geometry_count,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        };
        encoder.set_bytes(
            5,
            std::mem::size_of::<GpuClipParams>() as u64,
            &params as *const GpuClipParams as *const std::ffi::c_void,
        );

        let grid = MTLSize::new(self.geometry_count as u64, 1, 1);
        let threadgroup = MTLSize::new(1, 1, 1);
        encoder.dispatch_thread_groups(grid, threadgroup);

        encoder.end_encoding();
        command_buffer.commit();
        command_buffer.wait_until_completed();

        Ok(())
    }

    /// Dispatch clip kernel asynchronously — returns command buffer for later wait.
    pub fn dispatch_clip_async(
        &self,
        ctx: &Arc<MetalContext>,
    ) -> AccelResult<metal::CommandBuffer> {
        let command_buffer = ctx.command_queue().new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();

        let pipeline = {
            let mut cache = ctx.pipeline_cache();
            cache.get_pipeline(ctx.device(), "batch_clip")?.to_owned()
        };

        encoder.set_compute_pipeline_state(&pipeline);
        encoder.set_buffer(0, Some(self.coords.metal_buffer()), 0);
        encoder.set_buffer(1, Some(self.output_coords.metal_buffer()), 0);
        encoder.set_buffer(2, Some(self.descriptors.metal_buffer()), 0);
        encoder.set_buffer(3, Some(self.tile_infos.metal_buffer()), 0);
        encoder.set_buffer(4, Some(self.output_headers.metal_buffer()), 0);

        let params = GpuClipParams {
            geometry_count: self.geometry_count,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        };
        encoder.set_bytes(
            5,
            std::mem::size_of::<GpuClipParams>() as u64,
            &params as *const GpuClipParams as *const std::ffi::c_void,
        );

        let grid = MTLSize::new(self.geometry_count as u64, 1, 1);
        let threadgroup = MTLSize::new(1, 1, 1);
        encoder.dispatch_thread_groups(grid, threadgroup);

        encoder.end_encoding();
        command_buffer.commit();
        Ok(command_buffer.to_owned())
    }

    /// Read back output for a specific geometry index.
    pub fn read_output(&self, geom_idx: usize) -> Option<(Vec<f32>, u32)> {
        let header = &self.output_headers.as_slice()[geom_idx];
        let desc = &self.descriptors.as_slice()[geom_idx];

        if header.status != 0 || header.output_count == 0 {
            return None;
        }

        let count = header.output_count as usize;
        let offset = desc.output_offset as usize * 2;
        let end = offset + count * 2;
        let output = &self.output_coords.as_slice()[offset..end];

        Some((output.to_vec(), header.output_count))
    }
}
