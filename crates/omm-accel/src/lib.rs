pub mod error;

#[cfg(target_os = "macos")]
pub mod metal;

#[cfg(target_os = "macos")]
pub use metal::accelerator::GpuAccelerator;

/// Check if GPU acceleration is available on this platform.
pub fn is_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        metal::context::MetalContext::get().is_ok()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}
