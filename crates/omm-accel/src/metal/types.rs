/// Per-geometry descriptor for GPU processing.
/// Matches the Metal shader struct layout exactly.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GpuGeomDescriptor {
    pub coord_offset: u32,
    pub coord_count: u32,
    pub ring_offset: u32,
    pub ring_count: u32,
    pub geom_type: u32,
    pub output_offset: u32,
    pub output_capacity: u32,
    pub _pad: u32,
}

/// Per-geometry tile info for clipping + projection.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GpuTileInfo {
    pub clip_min_x: f32,
    pub clip_min_y: f32,
    pub clip_max_x: f32,
    pub clip_max_y: f32,
    pub n: f32,
    pub tx: f32,
    pub ty: f32,
    pub extent: f32,
}

/// Per-geometry output header written by GPU kernels.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GpuOutputHeader {
    pub output_count: u32,
    pub output_ring_count: u32,
    pub status: u32,
    pub _pad: u32,
}

/// Simplification parameters.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GpuSimplifyParams {
    pub geometry_count: u32,
    pub tolerance: f32,
    pub _pad0: u32,
    pub _pad1: u32,
}

/// Clipping parameters.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GpuClipParams {
    pub geometry_count: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
}

pub const GEOM_TYPE_POINT: u32 = 0;
pub const GEOM_TYPE_LINE: u32 = 1;
pub const GEOM_TYPE_POLYGON: u32 = 2;
