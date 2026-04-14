#include <metal_stdlib>
using namespace metal;

// ============================================================================
// GPU types — must match Rust #[repr(C)] structs exactly
// ============================================================================

struct GpuGeomDescriptor {
    uint coord_offset;
    uint coord_count;
    uint ring_offset;
    uint ring_count;
    uint geom_type;
    uint output_offset;
    uint output_capacity;
    uint _pad;
};

struct GpuTileInfo {
    float clip_min_x;
    float clip_min_y;
    float clip_max_x;
    float clip_max_y;
    float n;
    float tx;
    float ty;
    float extent;
};

struct GpuOutputHeader {
    atomic_uint output_count;
    uint output_ring_count;
    uint status;     // 0=ok, 1=overflow, 2=degenerate
    uint _pad;
};

struct GpuClipParams {
    uint geometry_count;
    uint _pad0;
    uint _pad1;
    uint _pad2;
};

constant uint GEOM_LINE = 1;
constant uint GEOM_POLYGON = 2;

// ============================================================================
// Sutherland-Hodgman: clip polygon against one edge
// ============================================================================

uint clip_edge(
    threadgroup float2* input,   uint in_count,
    threadgroup float2* output,
    float2 edge_start, float2 edge_normal
) {
    uint out_count = 0;
    if (in_count == 0) return 0;

    for (uint i = 0; i < in_count; i++) {
        float2 curr = input[i];
        float2 prev = input[(i + in_count - 1) % in_count];

        float curr_dot = dot(curr - edge_start, edge_normal);
        float prev_dot = dot(prev - edge_start, edge_normal);

        bool curr_inside = curr_dot >= 0.0f;
        bool prev_inside = prev_dot >= 0.0f;

        if (prev_inside && curr_inside) {
            output[out_count++] = curr;
        } else if (prev_inside && !curr_inside) {
            float t = prev_dot / (prev_dot - curr_dot);
            output[out_count++] = prev + t * (curr - prev);
        } else if (!prev_inside && curr_inside) {
            float t = prev_dot / (prev_dot - curr_dot);
            output[out_count++] = prev + t * (curr - prev);
            output[out_count++] = curr;
        }
    }
    return out_count;
}

// ============================================================================
// Clip kernel: operates on PRE-PROJECTED tile-local coordinates.
// One threadgroup per geometry. No projection — CPU did that already.
// ============================================================================

kernel void batch_clip(
    device const float*             coords      [[buffer(0)]],  // pre-projected tile-local
    device float*                   out_coords  [[buffer(1)]],
    device const GpuGeomDescriptor* descs       [[buffer(2)]],
    device const GpuTileInfo*       tiles       [[buffer(3)]],
    device GpuOutputHeader*         headers     [[buffer(4)]],
    constant GpuClipParams&         params      [[buffer(5)]],
    uint                            gid         [[threadgroup_position_in_grid]]
) {
    if (gid >= params.geometry_count) return;

    GpuGeomDescriptor desc = descs[gid];
    GpuTileInfo tile = tiles[gid];

    float min_x = tile.clip_min_x;
    float min_y = tile.clip_min_y;
    float max_x = tile.clip_max_x;
    float max_y = tile.clip_max_y;

    // Points: simple containment test
    if (desc.geom_type != GEOM_LINE && desc.geom_type != GEOM_POLYGON) {
        if (desc.coord_count == 1) {
            uint ci = desc.coord_offset * 2;
            float x = coords[ci], y = coords[ci + 1];
            if (x >= min_x && x <= max_x && y >= min_y && y <= max_y) {
                uint out_base = desc.output_offset * 2;
                out_coords[out_base] = x;
                out_coords[out_base + 1] = y;
                atomic_store_explicit(&headers[gid].output_count, 1, memory_order_relaxed);
                headers[gid].status = 0;
            } else {
                atomic_store_explicit(&headers[gid].output_count, 0, memory_order_relaxed);
                headers[gid].status = 2;
            }
        }
        return;
    }

    uint n = desc.coord_count;
    if (n < 2) {
        atomic_store_explicit(&headers[gid].output_count, 0, memory_order_relaxed);
        headers[gid].status = 2;
        return;
    }

    // Lines: Cohen-Sutherland per segment
    if (desc.geom_type == GEOM_LINE) {
        uint out_idx = 0;
        uint out_base = desc.output_offset * 2;

        for (uint i = 0; i + 1 < n; i++) {
            uint ci = (desc.coord_offset + i) * 2;
            float x0 = coords[ci], y0 = coords[ci + 1];
            float x1 = coords[ci + 2], y1 = coords[ci + 3];

            uint code0 = ((x0 < min_x) ? 1u : 0u) | ((x0 > max_x) ? 2u : 0u) |
                         ((y0 < min_y) ? 4u : 0u) | ((y0 > max_y) ? 8u : 0u);
            uint code1 = ((x1 < min_x) ? 1u : 0u) | ((x1 > max_x) ? 2u : 0u) |
                         ((y1 < min_y) ? 4u : 0u) | ((y1 > max_y) ? 8u : 0u);

            bool accept = false;
            for (int iter = 0; iter < 8; iter++) {
                if ((code0 | code1) == 0u) { accept = true; break; }
                if ((code0 & code1) != 0u) { break; }

                uint code_out = (code0 != 0u) ? code0 : code1;
                float x, y;
                if (code_out & 8u) {
                    x = x0 + (x1 - x0) * (max_y - y0) / (y1 - y0); y = max_y;
                } else if (code_out & 4u) {
                    x = x0 + (x1 - x0) * (min_y - y0) / (y1 - y0); y = min_y;
                } else if (code_out & 2u) {
                    y = y0 + (y1 - y0) * (max_x - x0) / (x1 - x0); x = max_x;
                } else {
                    y = y0 + (y1 - y0) * (min_x - x0) / (x1 - x0); x = min_x;
                }

                if (code_out == code0) {
                    x0 = x; y0 = y;
                    code0 = ((x0<min_x)?1u:0u)|((x0>max_x)?2u:0u)|((y0<min_y)?4u:0u)|((y0>max_y)?8u:0u);
                } else {
                    x1 = x; y1 = y;
                    code1 = ((x1<min_x)?1u:0u)|((x1>max_x)?2u:0u)|((y1<min_y)?4u:0u)|((y1>max_y)?8u:0u);
                }
            }

            if (accept && out_idx + 2 <= desc.output_capacity) {
                out_coords[out_base + out_idx * 2] = x0;
                out_coords[out_base + out_idx * 2 + 1] = y0;
                out_idx++;
                out_coords[out_base + out_idx * 2] = x1;
                out_coords[out_base + out_idx * 2 + 1] = y1;
                out_idx++;
            }
        }

        atomic_store_explicit(&headers[gid].output_count, out_idx, memory_order_relaxed);
        headers[gid].status = (out_idx > 0) ? 0 : 2;
        return;
    }

    // Polygons: Sutherland-Hodgman against 4 edges
    const uint MAX_VERTS = 2048;
    threadgroup float2 buf_a[MAX_VERTS];
    threadgroup float2 buf_b[MAX_VERTS];

    uint vn = min(n, MAX_VERTS);

    // Load pre-projected coordinates
    for (uint i = 0; i < vn; i++) {
        uint ci = (desc.coord_offset + i) * 2;
        buf_a[i] = float2(coords[ci], coords[ci + 1]);
    }

    // Clip left (normal = +x)
    uint count = clip_edge(buf_a, vn, buf_b, float2(min_x, 0), float2(1, 0));
    // Clip right (normal = -x)
    count = clip_edge(buf_b, count, buf_a, float2(max_x, 0), float2(-1, 0));
    // Clip bottom (normal = +y)
    count = clip_edge(buf_a, count, buf_b, float2(0, min_y), float2(0, 1));
    // Clip top (normal = -y)
    count = clip_edge(buf_b, count, buf_a, float2(0, max_y), float2(0, -1));

    // Result is in buf_a
    if (count < 3) {
        atomic_store_explicit(&headers[gid].output_count, 0, memory_order_relaxed);
        headers[gid].status = 2;
        return;
    }

    if (count > desc.output_capacity) {
        atomic_store_explicit(&headers[gid].output_count, 0, memory_order_relaxed);
        headers[gid].status = 1;
        return;
    }

    uint out_base = desc.output_offset * 2;
    for (uint i = 0; i < count; i++) {
        out_coords[out_base + i * 2] = buf_a[i].x;
        out_coords[out_base + i * 2 + 1] = buf_a[i].y;
    }

    atomic_store_explicit(&headers[gid].output_count, count, memory_order_relaxed);
    headers[gid].output_ring_count = 1;
    headers[gid].status = 0;
}
