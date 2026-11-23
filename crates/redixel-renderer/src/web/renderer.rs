use std::sync::Arc;

use redixel_core::{RedixelError, TextureId};
use redixel_math::{Color, Vec2, Vec3};
use redixel_profile as profile;
use wgpu::{
    AdapterInfo, CommandEncoder, CommandEncoderDescriptor,
    CurrentSurfaceTexture::{Lost, Occluded, Outdated, Suboptimal, Success, Timeout, Validation},
    LoadOp, Operations, PresentMode, RenderPass, RenderPassColorAttachment, RenderPassDepthStencilAttachment,
    RenderPassDescriptor, StoreOp, Surface, SurfaceTexture, TextureView, TextureViewDescriptor,
};
use winit::{
    dpi::PhysicalSize,
    window::{self, Window},
};

use super::{
    batch::{Batch2dBuffers, MeshBatch},
    device::GpuDevice,
    pipeline::ShapePipeline,
    texture::TextureRegistry,
};
use crate::{
    config::{RendererConfig, frame_uniforms},
    quad::Batch2d,
    shader::GlobalUniforms,
};

/// Commands collected during `on_render`, submitted to the GPU in one pass.
pub struct DrawQueue {
    pub clear:    Color,
    pub batch_2d: Batch2d,
    pub batch_3d: MeshBatch,
}

/// High-level renderer. Owns the GPU device, the 2D and 3D pipelines, and their batches.
///
/// Interaction model:
/// 1. The `draw_*` methods (or `draw_queue_mut()`) fill `batch_2d` and `batch_3d`
/// 2. `render()` flushes the depth-tested 3D batch, then the painter-ordered 2D batch over it, and presents
pub struct Renderer {
    device:     GpuDevice,
    pipeline:   ShapePipeline,
    textures:   TextureRegistry,
    queue:      DrawQueue,
    buffers_2d: Batch2dBuffers,
}

impl Renderer {
    /// The registry is built before the pipelines because it owns the layout
    /// of bind group 1, which those pipelines declare.
    pub async fn new(window: Arc<dyn Window>, config: RendererConfig) -> Result<Self, RedixelError> {
        let device: GpuDevice = GpuDevice::new(window, &config).await?;

        let textures: TextureRegistry = TextureRegistry::new(&device.device, &device.queue);
        let pipeline: ShapePipeline = ShapePipeline::new(&device.device, device.config.format, textures.layout());

        Ok(Self {
            device,
            pipeline,
            textures,
            queue: DrawQueue {
                clear:    Color::rgb(0.1, 0.2, 0.3),
                batch_2d: Batch2d::default(),
                batch_3d: MeshBatch::new(),
            },
            buffers_2d: Batch2dBuffers::new(),
        })
    }

    /// Decodes `bytes` and uploads the image into the slot named by `id`.
    ///
    /// The runtime issued `id` to game code before this ran, so a failure has
    /// to leave the handle usable: the slot stays empty and draws against it
    /// render the checkerboard.
    pub fn load_texture(&mut self, id: TextureId, bytes: &[u8]) -> Result<(), RedixelError> {
        self.textures.upload(&self.device.device, &self.device.queue, id, bytes)
    }

    /// Drops the presentation surface to yield GPU resources back to the OS.
    ///
    /// Essential for mobile platforms (like Android) where the OS invalidates the
    /// native window when the app is minimized or the screen turns off.
    /// Failing to drop the surface will result in a panic during the next render.
    pub fn suspend(&mut self) {
        self.device.suspend();
    }

    /// Reconstructs the presentation surface using a valid window handle.
    ///
    /// Call this when the OS resumes the application and provides a new native window,
    /// ensuring the renderer is re-linked to the screen and can draw safely again.
    pub fn resume(&mut self, window: &Arc<dyn window::Window>) -> Result<(), RedixelError> {
        self.device.resume(window)
    }

    /// Resizes the swap chain. Call whenever the window surface changes.
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.device.resize(new_size);
    }

    /// Returns the current surface size in pixels.
    pub fn surface_size(&self) -> (u32, u32) {
        (self.device.config.width, self.device.config.height)
    }

    /// Describes the GPU backing this renderer.
    pub fn adapter_info(&self) -> &AdapterInfo {
        &self.device.adapter_info
    }

    /// The present mode the surface is configured with, which differs from the
    /// requested one when the surface does not support it.
    pub fn present_mode(&self) -> PresentMode {
        self.device.config.present_mode
    }

    /// Returns a mutable reference to the draw queue, for callers driving the
    /// renderer directly. Games never see it: their draw calls reach it as
    /// `DrawCommand`s dispatched by the runtime.
    pub fn draw_queue_mut(&mut self) -> &mut DrawQueue {
        &mut self.queue
    }

    /// Sets the clear colour for the next frame.
    pub fn set_clear_color(&mut self, color: Color) {
        self.queue.clear = color;
    }

    /// Queues a filled rectangle.
    pub fn draw_rect(&mut self, position: Vec2, size: Vec2, color: Color) {
        self.queue.batch_2d.push_rect(position, size, color);
    }

    /// Queues a textured rectangle.
    pub fn draw_sprite(&mut self, position: Vec2, size: Vec2, texture: TextureId, tint: Color) {
        self.queue.batch_2d.push_sprite(position, size, texture, tint);
    }

    /// Queues a filled triangle.
    pub fn draw_triangle(&mut self, p1: Vec2, p2: Vec2, p3: Vec2, color: Color) {
        self.queue.batch_2d.push_triangle(p1, p2, p3, color);
    }

    /// Exchanges the queued 2D batch with `batch`, so a caller that filled one
    /// hands it over without copying, and gets back an empty one that keeps its
    /// capacity.
    pub fn swap_batch_2d(&mut self, batch: &mut Batch2d) {
        std::mem::swap(&mut self.queue.batch_2d, batch);
        batch.clear();
    }

    /// Queues a filled triangle in 3D view space.
    pub fn draw_triangle_3d(&mut self, p1: Vec3, p2: Vec3, p3: Vec3, color: Color) {
        self.queue.batch_3d.draw_triangle_3d(p1, p2, p3, color);
    }

    /// Queues a textured triangle in 3D view space.
    pub fn draw_triangle_3d_textured(&mut self, points: [Vec3; 3], uvs: [Vec2; 3], texture: TextureId, tint: Color) {
        self.queue
            .batch_3d
            .draw_triangle_3d_textured(points, uvs, texture, tint);
    }

    /// Uploads this frame's [`GlobalUniforms`] (see [`frame_uniforms`]) to
    /// both uniform blocks.
    ///
    /// Private because [`render`](Self::render) always calls it with that
    /// frame's time: an upload from anywhere else would be overwritten before
    /// it could reach a draw call.
    fn update_uniforms(&self, time: f32) {
        let (w, h): (u32, u32) = self.surface_size();
        let (uniforms_2d, uniforms_3d): (GlobalUniforms, GlobalUniforms) = frame_uniforms(w, h, time);
        self.pipeline.globals_2d.update(&self.device.queue, &uniforms_2d);
        self.pipeline.globals_3d.update(&self.device.queue, &uniforms_3d);
    }

    /// Flushes all queued draw calls and presents the frame.
    ///
    /// 1. Uploads the frame's global uniforms to both blocks
    /// 2. Begins the render pass (clear colour + depth)
    /// 3. Flushes the 3D batch, then the 2D batch, each with its own pipeline
    ///    and uniform block
    /// 4. Submits commands and presents
    ///
    /// 3D goes first so 2D lands on top of the finished scene and blends
    /// against it rather than against the clear colour.
    ///
    /// The uniform upload is driven from here rather than left to the caller so
    /// no frame can be presented against a stale surface size.
    pub fn render(&mut self, time: f32) -> Result<(), RedixelError> {
        let Some(surface) = &self.device.surface else {
            return Ok(());
        };

        profile::phase("uniforms");
        self.update_uniforms(time);

        profile::phase("acquire");
        let output: SurfaceTexture = Self::get_surface_texture(surface)?;

        profile::phase("encode");
        let view: TextureView = output.texture.create_view(&TextureViewDescriptor::default());

        let mut encoder: CommandEncoder = self.device.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("REDIXEL_ENCODER"),
        });

        {
            let clear: wgpu::Color = self.queue.clear.into();

            let mut pass: RenderPass<'_> = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("REDIXEL_RENDER_PASS"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view:           &view,
                    depth_slice:    None,
                    resolve_target: None,
                    ops:            Operations {
                        load:  LoadOp::Clear(clear),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view:        &self.device.depth_view,
                    depth_ops:   Some(Operations {
                        load:  LoadOp::Clear(1.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline.pipeline_3d);
            pass.set_bind_group(0, &self.pipeline.globals_3d.bind_group, &[]);
            self.queue
                .batch_3d
                .flush(&self.device.device, &self.device.queue, &mut pass, &self.textures);

            pass.set_pipeline(&self.pipeline.pipeline_2d);
            pass.set_bind_group(0, &self.pipeline.globals_2d.bind_group, &[]);
            self.buffers_2d.flush(
                &self.device.device,
                &self.device.queue,
                &mut pass,
                &self.pipeline,
                &self.textures,
                &mut self.queue.batch_2d,
            );
        }

        profile::phase("submit");
        self.device.queue.submit(std::iter::once(encoder.finish()));

        profile::phase("present");
        self.device.queue.present(output);

        Ok(())
    }

    fn get_surface_texture(surface: &Surface) -> Result<SurfaceTexture, RedixelError> {
        match surface.get_current_texture() {
            Success(texture) | Suboptimal(texture) => Ok(texture),
            Outdated | Lost => Err(RedixelError::SurfaceNeedsReconfiguration),
            Timeout | Occluded => Err(RedixelError::SurfaceIgnored),
            Validation => Err(RedixelError::SurfaceValidation),
        }
    }
}
