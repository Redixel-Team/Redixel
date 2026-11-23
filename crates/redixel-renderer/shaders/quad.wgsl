// Redixel — Instanced Quad Shader
//
// One instance per axis-aligned rectangle or sprite. The vertex shader expands
// it into the four corners of a triangle strip from `vertex_index`, in the
// order 0 = top-left, 1 = top-right, 2 = bottom-left, 3 = bottom-right, which
// matches the corner order `quad_vertices` builds for the indexed path.
//
// The bindings and the fragment stage are those of `shape.wgsl`, so both
// pipelines share one layout and one bind group per texture.
struct GlobalUniforms {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    resolution: vec2<f32>,
    time: f32,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> globals: GlobalUniforms;

@group(1) @binding(0)
var sprite_texture: texture_2d<f32>;

@group(1) @binding(1)
var sprite_sampler: sampler;

struct QuadIn {
    @location(0) position: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) uv_min: vec2<f32>,
    @location(4) uv_max: vec2<f32>,
}

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_quad(@builtin(vertex_index) corner_index: u32, quad: QuadIn) -> VertexOut {
    let corner: vec2<f32> = vec2<f32>(f32(corner_index & 1u), f32(corner_index >> 1u));
    let position: vec2<f32> = quad.position + corner * quad.size;

    var out: VertexOut;
    out.clip_pos = globals.projection * globals.view * vec4<f32>(position, 0.0, 1.0);
    out.color = quad.color;
    out.uv = mix(quad.uv_min, quad.uv_max, corner);
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(sprite_texture, sprite_sampler, in.uv) * in.color;
}
