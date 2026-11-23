//! WGPU-backed renderer for the Redixel engine: batched 2D shapes and sprites, depth-tested 3D triangles, and textures.

pub mod batch;
pub mod device;
pub mod pipeline;
pub mod quad;
pub mod renderer;
pub mod texture;

pub use batch::MeshBatch;
pub use pipeline::{GlobalUniforms, ShapePipeline, UniformBlock, Vertex};
pub use quad::{Batch2d, QuadInstance};
pub use renderer::{DrawQueue, Renderer, RendererConfig};
pub use texture::TextureRegistry;
