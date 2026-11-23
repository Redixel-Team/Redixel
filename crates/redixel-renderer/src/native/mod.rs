//! The native renderer, built directly on `wgpu-hal`.
//!
//! [`Renderer`] keeps the CPU-side batches and hands them to a [`Backend`]
//! once per frame: a [`Gpu`] over whichever `wgpu-hal` API opened first. `Gpu`
//! is generic over the API; what is not generic lives behind [`api::ApiExt`].

mod api;
mod gpu;
mod pipelines;
mod present;
mod stream;
mod textures;

use std::sync::Arc;

use redixel_core::{RedixelError, TextureId};
use redixel_math::{Color, Vec2, Vec3};
use wgpu_types::{AdapterInfo, Backends, PresentMode};
use winit::{dpi::PhysicalSize, window::Window};

use self::{
    api::{ApiExt, gpu_err},
    gpu::{Backend, FrameInput, Gpu},
};
use crate::{
    config::RendererConfig,
    geometry::{Geometry, TRIANGLE_INDICES, triangle_vertices_3d},
    quad::Batch2d,
};

/// High-level renderer: queue draw calls with the `draw_*` methods, then
/// [`render`](Self::render) draws the depth-tested 3D geometry, the
/// painter-ordered 2D batch over it, and presents.
pub struct Renderer {
    gpu:         Box<dyn Backend>,
    clear:       Color,
    batch_2d:    Batch2d,
    geometry_3d: Geometry,
}

impl Renderer {
    pub async fn new(window: Arc<dyn Window>, config: RendererConfig) -> Result<Self, RedixelError> {
        Ok(Self {
            gpu:         open_backend(window, &config)?,
            clear:       Color::rgb(0.1, 0.2, 0.3),
            batch_2d:    Batch2d::default(),
            geometry_3d: Geometry::default(),
        })
    }

    /// Decodes `bytes` and uploads the image into the slot named by `id`.
    ///
    /// The runtime issued `id` to game code before this ran, so a failure
    /// leaves the slot empty and draws against it render the checkerboard.
    pub fn load_texture(&mut self, id: TextureId, bytes: &[u8]) -> Result<(), RedixelError> {
        self.gpu.load_texture(id, bytes)
    }

    /// Drops the presentation surface, as mobile platforms require when the
    /// app goes to the background.
    pub fn suspend(&mut self) {
        self.gpu.suspend();
    }

    /// Recreates the presentation surface for the window the OS handed back.
    pub fn resume(&mut self, window: &Arc<dyn Window>) -> Result<(), RedixelError> {
        self.gpu.resume(window)
    }

    /// Resizes the swap chain. Call whenever the window surface changes.
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.gpu.resize(new_size);
    }

    pub fn surface_size(&self) -> (u32, u32) {
        self.gpu.surface_size()
    }

    pub fn adapter_info(&self) -> &AdapterInfo {
        self.gpu.adapter_info()
    }

    /// The present mode the surface is configured with, which differs from the
    /// requested one when the surface does not support it.
    pub fn present_mode(&self) -> PresentMode {
        self.gpu.present_mode()
    }

    pub fn set_clear_color(&mut self, color: Color) {
        self.clear = color;
    }

    pub fn draw_rect(&mut self, position: Vec2, size: Vec2, color: Color) {
        self.batch_2d.push_rect(position, size, color);
    }

    pub fn draw_sprite(&mut self, position: Vec2, size: Vec2, texture: TextureId, tint: Color) {
        self.batch_2d.push_sprite(position, size, texture, tint);
    }

    pub fn draw_triangle(&mut self, p1: Vec2, p2: Vec2, p3: Vec2, color: Color) {
        self.batch_2d.push_triangle(p1, p2, p3, color);
    }

    /// Exchanges the queued 2D batch with `batch`, so a caller that filled one
    /// hands it over without copying, and gets back an empty one that keeps its
    /// capacity.
    pub fn swap_batch_2d(&mut self, batch: &mut Batch2d) {
        std::mem::swap(&mut self.batch_2d, batch);
        batch.clear();
    }

    pub fn draw_triangle_3d(&mut self, p1: Vec3, p2: Vec3, p3: Vec3, color: Color) {
        self.geometry_3d.push(
            &triangle_vertices_3d([p1, p2, p3], [Vec2::ZERO; 3], color),
            &TRIANGLE_INDICES,
            None,
        );
    }

    pub fn draw_triangle_3d_textured(&mut self, points: [Vec3; 3], uvs: [Vec2; 3], texture: TextureId, tint: Color) {
        self.geometry_3d
            .push(&triangle_vertices_3d(points, uvs, tint), &TRIANGLE_INDICES, Some(texture));
    }

    /// Draws everything queued since the last call and presents it. The queues
    /// are emptied whether or not the frame reached the screen.
    pub fn render(&mut self, time: f32) -> Result<(), RedixelError> {
        self.gpu.render(
            FrameInput {
                clear:       self.clear,
                batch_2d:    &mut self.batch_2d,
                geometry_3d: &mut self.geometry_3d,
            },
            time,
        )
    }
}

type Opener = fn(Arc<dyn Window>, &RendererConfig) -> Result<Box<dyn Backend>, RedixelError>;

fn open<A: ApiExt>(window: Arc<dyn Window>, config: &RendererConfig) -> Result<Box<dyn Backend>, RedixelError> {
    Ok(Box::new(Gpu::<A>::new(window, config)?))
}

/// The APIs this target compiles in, in the order they are tried.
const BACKENDS: &[(Backends, Opener)] = &[
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "windows"))]
    (Backends::VULKAN, open::<wgpu_hal::api::Vulkan>),
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    (Backends::METAL, open::<wgpu_hal::api::Metal>),
    #[cfg(target_os = "windows")]
    (Backends::DX12, open::<wgpu_hal::api::Dx12>),
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "windows"))]
    (Backends::GL, open::<wgpu_hal::api::Gles>),
];

/// Opens the first API in [`BACKENDS`] that `config.backends` allows and that
/// can present to `window`, logging each one that fails before moving on.
fn open_backend(window: Arc<dyn Window>, config: &RendererConfig) -> Result<Box<dyn Backend>, RedixelError> {
    let mut last: Option<RedixelError> = None;

    for (backend, opener) in BACKENDS {
        if !config.backends.contains(*backend) {
            continue;
        }
        match opener(Arc::clone(&window), config) {
            Ok(gpu) => return Ok(gpu),
            Err(e) => {
                log::warn!("{backend:?} backend unavailable: {e}");
                last = Some(e);
            }
        }
    }

    Err(last.unwrap_or_else(|| gpu_err(format!("no compiled-in backend among {:?}", config.backends))))
}
