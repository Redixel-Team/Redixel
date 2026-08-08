// Redixel — Shape / Sprite Batch Shader
//
// Draws triangles/quads from a per-vertex colour attribute, modulated by a
// texture sample. Agnostic to which camera is bound: the caller binds the
// orthographic block before flushing 2D geometry and the perspective one
// before 3D, both against this same pipeline.
//
// Equally agnostic to whether anything is actually textured. Untextured
// geometry is bound against a 1x1 opaque white texture, making the multiply an
// identity, which is what lets solid shapes and sprites share one pipeline and
// one painter order.
//
// `view` is the identity in both blocks today — the engine has no camera
// transform, and `draw_triangle_3d` takes coordinates already in view space —
// so the multiply below is the seam a real camera slots into, not an accident.
//
// `glow` scales a time-driven brightness modulation, phased from the vertex
// position: reusing a datum that already exists per vertex is what lets
// separate objects pulse out of step with each other without any per-object
// uniform. The phase must vary *continuously* with position — a hash would
// hand moving geometry a new random phase every frame and render as strobing
// noise, not a pulse. Smooth phase means waves of brightness roll across large
// surfaces, while objects further apart than a fraction of a unit still pulse
// independently because the phase wraps many times over the distance. Two
// incommensurate frequencies keep the pulse from reading as a metronome. At
// `glow == 0.0` the modulation is the identity, so frames that never ask for
// it are untouched.
//
// Field order mirrors the Rust `GlobalUniforms`; the two layouts have to agree
// byte for byte.
struct GlobalUniforms {
    view: mat4x4<f32>,
    projection: mat4x4<f32>,
    resolution: vec2<f32>,
    time: f32,
    glow: f32,
}

@group(0) @binding(0)
var<uniform> globals: GlobalUniforms;

@group(1) @binding(0)
var sprite_texture: texture_2d<f32>;

@group(1) @binding(1)
var sprite_sampler: sampler;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
}

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) glow_phase: f32,
}

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip_pos = globals.projection * globals.view * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    out.uv = in.uv;
    out.glow_phase = dot(in.position, vec3<f32>(1.7, 2.3, 1.1));
    return out;
}

// The texture is an `Rgba8UnormSrgb`, so the sample arrives linearised. No
// gamma correction belongs here: the sRGB surface encodes on write-out.
@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let base: vec4<f32> = textureSample(sprite_texture, sprite_sampler, in.uv) * in.color;
    let wave_a: f32 = sin(globals.time * 2.1 + in.glow_phase);
    let wave_b: f32 = sin(globals.time * 3.7 + in.glow_phase * 1.9);
    let pulse: f32 = 0.5 + 0.25 * wave_a + 0.25 * wave_b;
    let brightness: f32 = mix(1.0, pulse, globals.glow);
    return vec4<f32>(base.rgb * brightness, base.a);
}
