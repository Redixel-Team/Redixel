//! The Redixel renderer: batched 2D shapes and sprites (drawn as GPU
//! instances), depth-tested 3D triangles, and textures.
//!
//! What the CPU builds and the shaders read is independent of any GPU API and
//! shared by every backend; [`Renderer`] itself currently runs on `wgpu`.

mod config;
mod decode;
mod geometry;
mod quad;
mod shader;
mod web;

pub use config::RendererConfig;
pub use quad::{Batch2d, QuadInstance};
pub use shader::{GlobalUniforms, Vertex};
pub use web::{DrawQueue, MeshBatch, Renderer, ShapePipeline, TextureRegistry, UniformBlock};
