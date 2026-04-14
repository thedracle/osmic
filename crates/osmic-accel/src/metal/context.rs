use std::sync::{Arc, OnceLock};

use metal::Device;
use parking_lot::Mutex;
use tracing::info;

use crate::error::{AccelError, AccelResult};

use super::pipeline_cache::PipelineCache;

static CONTEXT: OnceLock<Result<Arc<MetalContext>, String>> = OnceLock::new();

pub struct MetalContext {
    device: Device,
    command_queue: metal::CommandQueue,
    pipeline_cache: Mutex<PipelineCache>,
}

unsafe impl Send for MetalContext {}
unsafe impl Sync for MetalContext {}

impl MetalContext {
    pub fn get() -> AccelResult<Arc<MetalContext>> {
        let result = CONTEXT.get_or_init(|| Self::init().map_err(|e| e.to_string()));
        match result {
            Ok(ctx) => Ok(Arc::clone(ctx)),
            Err(e) => Err(AccelError::MetalInit(e.clone())),
        }
    }

    fn init() -> AccelResult<Arc<MetalContext>> {
        let device = Device::system_default()
            .ok_or_else(|| AccelError::MetalInit("No Metal device found".into()))?;

        info!(device = %device.name(), "Metal GPU initialized");

        let command_queue = device.new_command_queue();

        let library_bytes: &[u8] =
            include_bytes!(concat!(env!("OUT_DIR"), "/osmic_geometry.metallib"));

        let pipeline_cache = PipelineCache::new(&device, library_bytes)?;

        Ok(Arc::new(MetalContext {
            device,
            command_queue,
            pipeline_cache: Mutex::new(pipeline_cache),
        }))
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn command_queue(&self) -> &metal::CommandQueue {
        &self.command_queue
    }

    pub fn pipeline_cache(&self) -> parking_lot::MutexGuard<'_, PipelineCache> {
        self.pipeline_cache.lock()
    }
}
