//! The renderer over the `wgpu` crate: the device, the pipelines, the GPU side
//! of the batches, the texture registry and the frame loop.
//!
//! On the web this is the only renderer, reaching the browser's WebGPU or
//! WebGL2. Native targets use it too until the `wgpu-hal` backend replaces it
//! there.

mod batch;
mod device;
mod pipeline;
mod renderer;
mod texture;

pub use batch::MeshBatch;
pub use pipeline::{ShapePipeline, UniformBlock};
pub use renderer::{DrawQueue, Renderer};
pub use texture::TextureRegistry;
