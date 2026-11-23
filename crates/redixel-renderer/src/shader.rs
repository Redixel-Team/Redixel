//! What the shaders and the CPU agree on, independent of any GPU API: the
//! vertex and instance layouts, the global uniform block, the WGSL sources and
//! the depth states.

use std::mem::offset_of;

use wgpu_types::{
    CompareFunction, DepthBiasState, DepthStencilState, StencilState, TextureFormat, VertexAttribute, VertexFormat,
};

/// Draws triangles and quads from a per-vertex colour and texture coordinate,
/// for both the 2D and the 3D path.
pub(crate) const SHAPE_SHADER_SRC: &str = include_str!("../shaders/shape.wgsl");

/// Expands each [`QuadInstance`](crate::QuadInstance) into the four corners of
/// a strip.
pub(crate) const QUAD_SHADER_SRC: &str = include_str!("../shaders/quad.wgsl");

pub(crate) const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// A single vertex in the shape batch: 3D position + RGBA colour + texture
/// coordinate.
///
/// 2D draw calls carry `z = 0.0` through; only `draw_triangle_3d` supplies a
/// real z. Untextured geometry carries `uv = [0.0, 0.0]` and samples the
/// registry's 1×1 white pixel, so every vertex goes through the same
/// sample-and-multiply, textured or not.
///
/// `repr(C)` + packed fields → safe to cast to `&[u8]` via `bytemuck`.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color:    [f32; 4],
    pub uv:       [f32; 2],
}

impl Vertex {
    /// `shape.wgsl`'s `VertexIn`, location by location.
    pub(crate) const ATTRIBUTES: [VertexAttribute; 3] = [
        VertexAttribute {
            format:          VertexFormat::Float32x3,
            offset:          offset_of!(Vertex, position) as u64,
            shader_location: 0,
        },
        VertexAttribute {
            format:          VertexFormat::Float32x4,
            offset:          offset_of!(Vertex, color) as u64,
            shader_location: 1,
        },
        VertexAttribute {
            format:          VertexFormat::Float32x2,
            offset:          offset_of!(Vertex, uv) as u64,
            shader_location: 2,
        },
    ];
}

/// Everything the vertex shader reads from `group(0) binding(0)`: the two
/// column-major 4×4 camera matrices, plus the per-frame globals `resolution`
/// and `time`. The group is visible to the vertex stage only, so a fragment
/// effect reading them needs its visibility widened where the layout is built.
///
/// The field order is the memory layout WGSL expects, so it is not free to
/// change: matrices align to 16 bytes and sit first, `resolution` is a
/// `vec2<f32>` (8-byte aligned, unlike a `vec3` which would align to 16 and need
/// padding of its own), and `time` plus `_padding` close the struct at 144
/// bytes — a multiple of the 16-byte struct alignment WGSL imposes on a struct
/// containing a `mat4x4`. Leaving that padding implicit is what lets the Rust
/// and WGSL layouts silently drift apart, so it is spelled out here instead.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobalUniforms {
    pub view:       [[f32; 4]; 4],
    pub projection: [[f32; 4]; 4],
    pub resolution: [f32; 2],
    pub time:       f32,
    pub _padding:   f32,
}

/// Depth state for 3D geometry: a real depth test, written to the buffer.
///
/// `LessEqual` rather than `Less` so coplanar geometry resolves in submission
/// order instead of the second surface failing its own test.
pub(crate) fn depth_3d() -> DepthStencilState {
    DepthStencilState {
        format:              DEPTH_FORMAT,
        depth_write_enabled: Some(true),
        depth_compare:       Some(CompareFunction::LessEqual),
        stencil:             StencilState::default(),
        bias:                DepthBiasState::default(),
    }
}

/// Depth state for 2D geometry: no test, no write.
///
/// 2D content is painter-ordered, so later draw calls belong on top regardless
/// of the buffer, and writing to it would let a flat overlay occlude the 3D
/// scene behind it.
pub(crate) fn depth_2d() -> DepthStencilState {
    DepthStencilState {
        format:              DEPTH_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare:       Some(CompareFunction::Always),
        stencil:             StencilState::default(),
        bias:                DepthBiasState::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_attributes_cover_the_whole_vertex() {
        let last: &VertexAttribute = &Vertex::ATTRIBUTES[2];
        assert_eq!(last.offset + last.format.size(), std::mem::size_of::<Vertex>() as u64);
    }

    #[test]
    fn global_uniforms_close_at_144_bytes() {
        assert_eq!(std::mem::size_of::<GlobalUniforms>(), 144);
    }
}
