//! The Redixel renderer: batched 2D shapes and sprites (drawn as GPU
//! instances), depth-tested 3D triangles, and textures.
//!
//! What the CPU builds and the shaders read is independent of any GPU API and
//! shared by every backend. [`Renderer`] runs natively on `wgpu-hal` (Vulkan,
//! Metal, DX12 or GLES) and on the web on `wgpu` (WebGPU or WebGL2).

mod config;
mod decode;
mod geometry;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod quad;
mod shader;
#[cfg(target_arch = "wasm32")]
mod web;

pub use config::{DEFAULT_PRESENT_SPIN, RendererConfig};
#[cfg(not(target_arch = "wasm32"))]
pub use native::Renderer;
pub use quad::{Batch2d, QuadInstance};
pub use shader::{GlobalUniforms, Vertex};
#[cfg(target_arch = "wasm32")]
pub use web::{DrawQueue, MeshBatch, Renderer, ShapePipeline, TextureRegistry, UniformBlock};
