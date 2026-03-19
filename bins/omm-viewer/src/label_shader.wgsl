struct LabelUniforms {
    offset: vec2<f32>,
};

@group(0) @binding(0) var t_labels: texture_2d<f32>;
@group(0) @binding(1) var s_labels: sampler;
@group(0) @binding(2) var<uniform> label_uniforms: LabelUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 4>(
        vec2(-1.0, -1.0),
        vec2( 1.0, -1.0),
        vec2(-1.0,  1.0),
        vec2( 1.0,  1.0),
    );
    var uvs = array<vec2<f32>, 4>(
        vec2(0.0, 1.0),
        vec2(1.0, 1.0),
        vec2(0.0, 0.0),
        vec2(1.0, 0.0),
    );
    var out: VertexOutput;
    out.position = vec4(positions[vi], 0.0, 1.0);
    // Shift UVs to track camera pan since last label render
    out.uv = uvs[vi] + label_uniforms.offset;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Discard pixels outside [0,1] UV range (scrolled off edge)
    if (in.uv.x < 0.0 || in.uv.x > 1.0 || in.uv.y < 0.0 || in.uv.y > 1.0) {
        return vec4(0.0, 0.0, 0.0, 0.0);
    }
    return textureSample(t_labels, s_labels, in.uv);
}
