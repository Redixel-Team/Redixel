//! The Redixel renderer: batched 2D shapes and sprites (drawn as GPU
//! instances), depth-tested 3D triangles, and textures.
//!
//! What the CPU builds and the shaders read is independent of any GPU API and
//! shared by every backend. [`Renderer`] runs on `wgpu`, or natively on
//! `wgpu-hal` with the `hal` feature.

mod config;
mod decode;
mod geometry;
#[cfg(all(feature = "hal", not(target_arch = "wasm32")))]
mod native;
mod quad;
mod shader;
mod web;

pub use config::{DEFAULT_PRESENT_SPIN, RendererConfig};
#[cfg(all(feature = "hal", not(target_arch = "wasm32")))]
pub use native::Renderer;
pub use quad::{Batch2d, QuadInstance};
pub use shader::{GlobalUniforms, Vertex};
/// The `wgpu` renderer, kept reachable beside the native one while the
/// `hal` feature is transitional, so the two can be compared.
#[cfg(all(feature = "hal", not(target_arch = "wasm32")))]
pub use web::Renderer as WgpuRenderer;
#[cfg(not(all(feature = "hal", not(target_arch = "wasm32"))))]
pub use web::Renderer;
pub use web::{DrawQueue, MeshBatch, ShapePipeline, TextureRegistry, UniformBlock};
