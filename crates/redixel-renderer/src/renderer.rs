use std::sync::Arc;

use wgpu::{
    Backends, CommandEncoder, CommandEncoderDescriptor,
    CurrentSurfaceTexture::{Lost, Occluded, Outdated, Suboptimal, Success, Timeout, Validation},
    LoadOp, Operations, PresentMode, RenderPass, RenderPassColorAttachment, RenderPassDepthStencilAttachment,
    RenderPassDescriptor, StoreOp, Surface, SurfaceTexture, TextureView, TextureViewDescriptor,
};

use winit::{
    dpi::PhysicalSize,
    window::{self, Window},
};

use redixel_core::{RedixelError, TextureId};
use redixel_math::{Color, Mat4, Vec2, Vec3};

use crate::{
    batch::{MeshBatch, SpriteBatch},
    device::GpuDevice,
    pipeline::{GlobalUniforms, ShapePipeline},
    texture::TextureRegistry,
};

const CAMERA_FOV_Y_DEGREES: f32 = 60.0;
const CAMERA_NEAR: f32 = 0.1;
const CAMERA_FAR: f32 = 100.0;

/// All renderer settings resolved from `config.json` by `redixel-runtime`
/// and injected at construction time. The renderer never touches the config system.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    pub backends: Backends,
    pub present_mode: PresentMode,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            backends: Backends::all(),
            present_mode: PresentMode::AutoVsync,
        }
    }
}

/// Commands collected during `on_render`, submitted to the GPU in one pass.
pub struct DrawQueue {
    pub clear: Color,
    pub glow: f32,
    pub batch_2d: SpriteBatch,
    pub batch_3d: MeshBatch,
}

/// High-level renderer. Owns the GPU device, shape pipeline, and sprite batch.
///
/// Interaction model:
/// 1. `begin_frame()` — resets the draw queue, returns a `&mut DrawQueue`
/// 2. Game code calls `queue.batch.draw_rect(...)` freely
/// 3. `end_frame()` — flushes the queue and presents the frame
pub struct Renderer {
    device: GpuDevice,
    pipeline: ShapePipeline,
    textures: TextureRegistry,
    queue: DrawQueue,
}

impl Renderer {
    /// The registry is built before the pipelines because it owns the layout
    /// of bind group 1, which those pipelines declare.
    pub async fn new(window: Arc<dyn Window>, config: RendererConfig) -> Result<Self, RedixelError> {
        let device: GpuDevice = GpuDevice::new(window, &config).await?;

        let textures: TextureRegistry = TextureRegistry::new(&device.device, &device.queue);
        let pipeline: ShapePipeline = ShapePipeline::new(&device.device, device.config.format, textures.layout());

        let batch_2d: SpriteBatch = SpriteBatch::new(&device.device);
        let batch_3d: MeshBatch = MeshBatch::new(&device.device);

        Ok(Self {
            device,
            pipeline,
            textures,
            queue: DrawQueue {
                clear: Color::rgb(0.1, 0.2, 0.3),
                glow: 0.0,
                batch_2d,
                batch_3d,
            },
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

    /// Returns a mutable reference to the draw queue.
    /// Game code queues draw calls here during `on_render`.
    pub fn draw_queue_mut(&mut self) -> &mut DrawQueue {
        &mut self.queue
    }

    /// Sets the clear colour for the next frame.
    pub fn set_clear_color(&mut self, color: Color) {
        self.queue.clear = color;
    }

    /// Sets the strength of the shader's time-driven brightness modulation for
    /// the next frame's 3D batch. `0.0` — the default — disables it entirely;
    /// the 2D batch always renders with it disabled.
    pub fn set_glow(&mut self, amount: f32) {
        self.queue.glow = amount;
    }

    /// Queues a filled rectangle.
    pub fn draw_rect(&mut self, position: Vec2, size: Vec2, color: Color) {
        self.queue.batch_2d.draw_rect(position, size, color);
    }

    /// Queues a textured rectangle.
    pub fn draw_sprite(&mut self, position: Vec2, size: Vec2, texture: TextureId, tint: Color) {
        self.queue.batch_2d.draw_sprite(position, size, texture, tint);
    }

    /// Queues a filled triangle.
    pub fn draw_triangle(&mut self, p1: Vec2, p2: Vec2, p3: Vec2, color: Color) {
        self.queue.batch_2d.draw_triangle(p1, p2, p3, color);
    }

    /// Queues a filled triangle in 3D view space.
    pub fn draw_triangle_3d(&mut self, p1: Vec3, p2: Vec3, p3: Vec3, color: Color) {
        self.queue.batch_3d.draw_triangle_3d(p1, p2, p3, color);
    }

    /// Queues a triangle in 3D view space with an independent colour per
    /// vertex, interpolated across the face.
    pub fn draw_triangle_3d_shaded(&mut self, points: [Vec3; 3], colors: [Color; 3]) {
        self.queue.batch_3d.draw_triangle_3d_shaded(points, colors);
    }

    /// Queues a textured triangle in 3D view space.
    pub fn draw_triangle_3d_textured(&mut self, points: [Vec3; 3], uvs: [Vec2; 3], texture: TextureId, tint: Color) {
        self.queue
            .batch_3d
            .draw_triangle_3d_textured(points, uvs, texture, tint);
    }

    /// Uploads this frame's [`GlobalUniforms`] to both uniform blocks.
    ///
    /// The matrices are rebuilt from the surface configuration every frame,
    /// which is what keeps geometry proportional across a resize without the
    /// resize path having to re-upload anything. `resolution` carries those
    /// same dimensions to the shader, where the projection is no help.
    ///
    /// `view` is the identity in both blocks: the engine has no camera
    /// transform yet, and 3D draw calls arrive already in view space.
    ///
    /// `glow` reaches only the 3D block — it modulates scene brightness, and 2D
    /// content (HUDs, overlays) is exactly what should not throb with the
    /// scene — so the 2D block always carries `0.0`, the identity.
    ///
    /// `time` is seconds since startup, narrowed to `f32` only here — WGSL
    /// uniforms have no `f64`. Its resolution decays to about a millisecond
    /// after a couple of hours of uptime; an effect needing better than that
    /// wants the time reduced modulo a multiple of `TAU` before this call, not
    /// a wider type.
    pub fn update_uniforms(&self, time: f32) {
        let (w, h): (u32, u32) = self.surface_size();
        let resolution: [f32; 2] = [w as f32, h as f32];

        let ortho: Mat4 = Mat4::orthographic(0.0, w as f32, h as f32, 0.0, -1.0, 1.0);
        let uniforms_2d: GlobalUniforms = GlobalUniforms {
            view: Mat4::IDENTITY.cols,
            projection: ortho.cols,
            resolution,
            time,
            glow: 0.0,
        };
        self.pipeline.globals_2d.update(&self.device.queue, &uniforms_2d);

        let aspect: f32 = w as f32 / h as f32;
        let perspective: Mat4 = Mat4::perspective(CAMERA_FOV_Y_DEGREES.to_radians(), aspect, CAMERA_NEAR, CAMERA_FAR);
        let uniforms_3d: GlobalUniforms = GlobalUniforms {
            view: Mat4::IDENTITY.cols,
            projection: perspective.cols,
            resolution,
            time,
            glow: self.queue.glow,
        };
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

        self.update_uniforms(time);

        let output: SurfaceTexture = Self::get_surface_texture(surface)?;
        let view: TextureView = output.texture.create_view(&TextureViewDescriptor::default());

        let mut encoder: CommandEncoder = self.device.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("REDIXEL_ENCODER"),
        });

        {
            let clear: wgpu::Color = self.queue.clear.into();

            let mut pass: RenderPass<'_> = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("REDIXEL_RENDER_PASS"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(clear),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &self.device.depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
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
            self.queue
                .batch_2d
                .flush(&self.device.device, &self.device.queue, &mut pass, &self.textures);
        }

        self.device.queue.submit(std::iter::once(encoder.finish()));
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
