use std::collections::HashMap;

use metal::{ComputePipelineState, Device, Library};

use crate::error::{AccelError, AccelResult};

pub struct PipelineCache {
    library: Library,
    pipelines: HashMap<String, ComputePipelineState>,
}

impl PipelineCache {
    pub fn new(device: &Device, metallib_bytes: &[u8]) -> AccelResult<Self> {
        let library = device
            .new_library_with_data(metallib_bytes)
            .map_err(|e| AccelError::ShaderCompilation(e.to_string()))?;

        Ok(Self {
            library,
            pipelines: HashMap::new(),
        })
    }

    pub fn get_pipeline(
        &mut self,
        device: &Device,
        function_name: &str,
    ) -> AccelResult<&ComputePipelineState> {
        if !self.pipelines.contains_key(function_name) {
            let function = self
                .library
                .get_function(function_name, None)
                .map_err(|e| {
                    AccelError::ShaderCompilation(format!(
                        "Function '{}' not found: {}",
                        function_name, e
                    ))
                })?;

            let pipeline = device
                .new_compute_pipeline_state_with_function(&function)
                .map_err(|e| AccelError::ShaderCompilation(e.to_string()))?;

            self.pipelines.insert(function_name.to_string(), pipeline);
        }

        Ok(&self.pipelines[function_name])
    }
}
