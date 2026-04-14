use metal::{Device, MTLResourceOptions};

use crate::error::{AccelError, AccelResult};

/// Type-safe Metal buffer backed by unified (shared) memory.
///
/// On Apple Silicon, CPU and GPU share physical RAM — zero DMA copies.
pub struct MetalBuffer<T: Copy> {
    buffer: metal::Buffer,
    len: usize,
    _phantom: std::marker::PhantomData<T>,
}

unsafe impl<T: Copy> Send for MetalBuffer<T> {}
unsafe impl<T: Copy> Sync for MetalBuffer<T> {}

impl<T: Copy> MetalBuffer<T> {
    /// Create an uninitialized buffer of `len` elements.
    pub fn new(device: &Device, len: usize) -> AccelResult<Self> {
        let byte_len = len * std::mem::size_of::<T>();
        if byte_len == 0 {
            return Err(AccelError::BufferCreation("Zero-length buffer".into()));
        }

        let buffer = device.new_buffer(
            byte_len as u64,
            MTLResourceOptions::StorageModeShared,
        );

        Ok(Self {
            buffer,
            len,
            _phantom: std::marker::PhantomData,
        })
    }

    /// Create a buffer initialized from a slice.
    pub fn from_slice(device: &Device, data: &[T]) -> AccelResult<Self> {
        let byte_len = data.len() * std::mem::size_of::<T>();
        if byte_len == 0 {
            return Err(AccelError::BufferCreation("Zero-length buffer".into()));
        }

        let buffer = device.new_buffer_with_data(
            data.as_ptr() as *const std::ffi::c_void,
            byte_len as u64,
            MTLResourceOptions::StorageModeShared,
        );

        Ok(Self {
            buffer,
            len: data.len(),
            _phantom: std::marker::PhantomData,
        })
    }

    /// Read the buffer contents as a slice.
    pub fn as_slice(&self) -> &[T] {
        let ptr = self.buffer.contents() as *const T;
        unsafe { std::slice::from_raw_parts(ptr, self.len) }
    }

    /// Write to the buffer contents as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let ptr = self.buffer.contents() as *mut T;
        unsafe { std::slice::from_raw_parts_mut(ptr, self.len) }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the underlying Metal buffer for binding to compute encoders.
    pub fn metal_buffer(&self) -> &metal::Buffer {
        &self.buffer
    }
}
