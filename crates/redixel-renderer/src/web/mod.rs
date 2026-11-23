//! The renderer over the `wgpu` crate: the device, the pipelines, the GPU side
//! of the batches, the texture registry and the frame loop.
//!
//! Built only for the web, where it reaches the browser's WebGPU or WebGL2;
//! native targets use [`crate::native`] instead.

mod batch;
mod device;
mod pipeline;
mod renderer;
mod texture;

pub use batch::MeshBatch;
pub use pipeline::{ShapePipeline, UniformBlock};
pub use renderer::{DrawQueue, Renderer};
pub use texture::TextureRegistry;
