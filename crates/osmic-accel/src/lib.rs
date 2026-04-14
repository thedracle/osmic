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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn is_available_false_off_macos() {
        assert!(!is_available());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn is_available_does_not_panic() {
        // Whether the Metal context is available depends on the build
        // environment (headless CI, sandboxes, etc.); just ensure the
        // probe is callable and returns a bool.
        let _ = is_available();
    }
}
