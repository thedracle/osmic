use thiserror::Error;

#[derive(Error, Debug)]
pub enum AccelError {
    #[error("Metal initialization failed: {0}")]
    MetalInit(String),

    #[error("Buffer creation failed: {0}")]
    BufferCreation(String),

    #[error("Shader compilation failed: {0}")]
    ShaderCompilation(String),

    #[error("Kernel execution failed: {0}")]
    ExecutionFailed(String),

    #[error("GPU timeout after {0:?}")]
    GpuTimeout(std::time::Duration),

    #[error("GPU not available")]
    NotAvailable,
}

pub type AccelResult<T> = Result<T, AccelError>;
