//! The GPU half of the native renderer, generic over the `wgpu-hal` API it
//! runs on.

use std::{borrow::Borrow, iter, mem::ManuallyDrop, num::NonZeroU64, sync::Arc, time::Duration};

use hal::{Adapter as _, CommandEncoder as _, Device as _, Instance as _, Queue as _, Surface as _};
use raw_window_handle::HasDisplayHandle as _;
use redixel_core::{RedixelError, TextureId};
use redixel_math::Color;
use redixel_profile as profile;
use wgpu_hal as hal;
use wgpu_types as wgt;
use winit::{dpi::PhysicalSize, window::Window};

use super::{
    api::{ApiExt, create_buffer, gpu_err, write_mapped},
    pipelines::Pipelines,
    present::{PresentJob, Presenter},
    stream::{Stream, StreamBuffer},
    textures::{GpuTexture, Textures, Upload},
};
use crate::{
    config::{RendererConfig, frame_uniforms},
    decode::{CHECKERBOARD_CELL, CHECKERBOARD_SIZE, WHITE_PIXEL_RGBA, checkerboard_rgba, decode_rgba},
    geometry::Geometry,
    quad::{Batch2d, RunKind},
    shader::{DEPTH_FORMAT, GlobalUniforms},
};

const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(1);

/// Frame slots the CPU may record ahead of the GPU.
const FRAMES_IN_FLIGHT: usize = 2;

/// Swapchain images the presentation engine may queue, within what the
/// surface allows. Fewer stalls the frame thread on present at high frame
/// rates.
const FRAME_LATENCY: u32 = 4;

/// A uniform buffer and the group-0 bind group exposing it.
struct Uniform<A: hal::Api> {
    buffer: A::Buffer,
    group:  A::BindGroup,
}

impl<A: hal::Api> Uniform<A> {
    fn new(device: &A::Device, layout: &A::BindGroupLayout, label: &str) -> Result<Self, RedixelError> {
        let size: u64 = size_of::<GlobalUniforms>() as u64;
        let buffer: A::Buffer =
            create_buffer::<A>(device, label, size, wgt::BufferUses::UNIFORM | wgt::BufferUses::MAP_WRITE)?;

        let binding: hal::BufferBinding<'_, A::Buffer> =
            hal::BufferBinding::new_unchecked(&buffer, 0, NonZeroU64::new(size));
        // SAFETY: the binding covers exactly the buffer just created, matching
        // `layout`'s single uniform entry.
        let group: Result<A::BindGroup, hal::DeviceError> = unsafe {
            device.create_bind_group(&hal::BindGroupDescriptor {
                label: Some(label),
                layout,
                buffers: &[binding],
                samplers: &[],
                textures: &[],
                acceleration_structures: &[],
                external_textures: &[],
                entries: &[hal::BindGroupEntry {
                    binding:        0,
                    resource_index: 0,
                    count:          1,
                }],
            })
        };

        match group {
            Ok(group) => Ok(Self { buffer, group }),
            Err(e) => {
                // SAFETY: nothing has used the buffer yet.
                unsafe { device.destroy_buffer(buffer) };
                Err(gpu_err(e))
            }
        }
    }

    /// # Safety
    ///
    /// No pending submission references this uniform.
    unsafe fn destroy(self, device: &A::Device) {
        unsafe {
            device.destroy_bind_group(self.group);
            device.destroy_buffer(self.buffer);
        }
    }
}

/// Everything one in-flight frame writes, reusable once `fence_value` has passed.
struct Frame<A: ApiExt> {
    encoder:     A::CommandEncoder,
    used:        Vec<A::CommandBuffer>,
    used_views:  Vec<A::TextureView>,
    fence_value: hal::FenceValue,
    globals_2d:  Uniform<A>,
    globals_3d:  Uniform<A>,
    stream_2d:   Stream<A>,
    stream_3d:   Stream<A>,
}

impl<A: ApiExt> Frame<A> {
    fn new(device: &A::Device, queue: &A::Queue, layout: &A::BindGroupLayout) -> Result<Self, RedixelError> {
        // SAFETY: the encoder is created for the queue it submits to.
        let encoder: A::CommandEncoder = unsafe {
            device.create_command_encoder(&hal::CommandEncoderDescriptor {
                label: Some("REDIXEL_FRAME_ENCODER"),
                queue,
            })
        }
        .map_err(gpu_err)?;

        Ok(Self {
            encoder,
            used: Vec::new(),
            used_views: Vec::new(),
            fence_value: 0,
            globals_2d: Uniform::new(device, layout, "REDIXEL_GLOBALS_2D")?,
            globals_3d: Uniform::new(device, layout, "REDIXEL_GLOBALS_3D")?,
            stream_2d: Stream::default(),
            stream_3d: Stream::default(),
        })
    }

    /// Returns the slot's command buffers to its encoder and destroys its
    /// per-frame views.
    ///
    /// # Safety
    ///
    /// The slot's fence value has passed.
    unsafe fn recycle(&mut self, device: &A::Device) {
        unsafe {
            self.encoder.reset_all(self.used.drain(..));
            for view in self.used_views.drain(..) {
                device.destroy_texture_view(view);
            }
        }
    }

    /// # Safety
    ///
    /// The device is idle.
    unsafe fn destroy(mut self, device: &A::Device) {
        unsafe {
            self.recycle(device);
            self.globals_2d.destroy(device);
            self.globals_3d.destroy(device);
            self.stream_2d.destroy(device);
            self.stream_3d.destroy(device);
        }
    }
}

struct Depth<A: hal::Api> {
    texture: A::Texture,
    view:    A::TextureView,
}

impl<A: hal::Api> Depth<A> {
    fn new(device: &A::Device, extent: wgt::Extent3d) -> Result<Self, RedixelError> {
        // SAFETY: plain allocation; the view is created against its own texture.
        unsafe {
            let texture: A::Texture = device
                .create_texture(&hal::TextureDescriptor {
                    label:           Some("REDIXEL_DEPTH"),
                    size:            extent,
                    mip_level_count: 1,
                    sample_count:    1,
                    dimension:       wgt::TextureDimension::D2,
                    format:          DEPTH_FORMAT,
                    usage:           wgt::TextureUses::DEPTH_STENCIL_WRITE,
                    memory_flags:    hal::MemoryFlags::empty(),
                    view_formats:    Vec::new(),
                })
                .map_err(gpu_err)?;

            match device.create_texture_view(
                &texture,
                &hal::TextureViewDescriptor {
                    label:     Some("REDIXEL_DEPTH_VIEW"),
                    format:    DEPTH_FORMAT,
                    dimension: wgt::TextureViewDimension::D2,
                    usage:     wgt::TextureUses::DEPTH_STENCIL_WRITE,
                    range:     wgt::ImageSubresourceRange::default(),
                },
            ) {
                Ok(view) => Ok(Self { texture, view }),
                Err(e) => {
                    device.destroy_texture(texture);
                    Err(gpu_err(e))
                }
            }
        }
    }

    /// # Safety
    ///
    /// No pending submission references the depth attachment.
    unsafe fn destroy(self, device: &A::Device) {
        unsafe {
            device.destroy_texture_view(self.view);
            device.destroy_texture(self.texture);
        }
    }
}

/// What [`Gpu::render`] draws: the CPU-side batches, consumed and cleared.
pub(super) struct FrameInput<'a> {
    pub(super) clear:       Color,
    pub(super) batch_2d:    &'a mut Batch2d,
    pub(super) geometry_3d: &'a mut Geometry,
}

/// The renderer's view of one GPU backend, so the API can be chosen at run
/// time.
pub(super) trait Backend: Send {
    /// Decodes `bytes` into `id`'s slot. The slot is emptied first, so a
    /// failed reload falls back to the checkerboard rather than keeping a
    /// stale image.
    fn load_texture(&mut self, id: TextureId, bytes: &[u8]) -> Result<(), RedixelError>;
    fn suspend(&mut self);
    fn resume(&mut self, window: &Arc<dyn Window>) -> Result<(), RedixelError>;
    fn resize(&mut self, new_size: PhysicalSize<u32>);
    fn surface_size(&self) -> (u32, u32);
    fn adapter_info(&self) -> &wgt::AdapterInfo;
    fn present_mode(&self) -> wgt::PresentMode;
    /// Records and presents one frame: 3D first, then 2D over it. The batches
    /// in `input` are cleared, drawn or not.
    fn render(&mut self, input: FrameInput<'_>, time: f32) -> Result<(), RedixelError>;
}

/// Synchronisation: one fence with a monotonic value. Each frame slot
/// remembers the value its last submission signals, and `render` waits for it
/// before touching anything the slot owns. Texture uploads wait for their own
/// submission; resize, suspend and drop wait for the whole queue.
///
/// Field order is drop order: the window outlives the surface created from it.
pub(super) struct Gpu<A: ApiExt> {
    frames:         Vec<Frame<A>>,
    current:        usize,
    upload_encoder: A::CommandEncoder,
    textures:       ManuallyDrop<Textures<A>>,
    pipelines:      ManuallyDrop<Pipelines<A>>,
    depth:          ManuallyDrop<Depth<A>>,
    depth_fresh:    bool,
    fence:          ManuallyDrop<A::Fence>,
    last_submitted: hal::FenceValue,
    surface_config: hal::SurfaceConfiguration,
    surface_views:  Vec<(u64, A::TextureView)>,
    presenter:      Presenter<A>,
    surface:        Option<Arc<A::Surface>>,
    queue:          Arc<A::Queue>,
    device:         A::Device,
    adapter_info:   wgt::AdapterInfo,
    max_texture:    u32,
    _adapter:       A::Adapter,
    instance:       A::Instance,
    _window:        Arc<dyn Window>,
}

impl<A: ApiExt> Gpu<A> {
    pub(super) fn new(window: Arc<dyn Window>, config: &RendererConfig) -> Result<Self, RedixelError> {
        // SAFETY: the display handle belongs to `window`, which `Gpu` keeps
        // alive past the instance.
        let instance: A::Instance = unsafe {
            A::Instance::init(&hal::InstanceDescriptor {
                name:                     "Redixel",
                flags:                    wgt::InstanceFlags::from_build_config().with_env(),
                memory_budget_thresholds: wgt::MemoryBudgetThresholds::default(),
                backend_options:          wgt::BackendOptions::default(),
                telemetry:                None,
                display:                  Some(window.display_handle().map_err(gpu_err)?),
            })
        }
        .map_err(gpu_err)?;

        let surface: A::Surface = create_surface::<A>(&instance, &window)?;
        let (exposed, caps): (hal::ExposedAdapter<A>, hal::SurfaceCapabilities) =
            pick_adapter::<A>(&instance, &surface)?;

        // SAFETY: the limits are the adapter's own.
        let hal::OpenDevice { device, queue }: hal::OpenDevice<A> = unsafe {
            exposed.adapter.open(
                wgt::Features::empty(),
                &exposed.capabilities.limits,
                &wgt::MemoryHints::Performance,
            )
        }
        .map_err(gpu_err)?;

        let surface_config: hal::SurfaceConfiguration =
            surface_config(&caps, window.surface_size(), config.present_mode)?;
        // SAFETY: the configuration was resolved against this surface's
        // capabilities on this adapter.
        unsafe { surface.configure(&device, &surface_config) }.map_err(gpu_err)?;

        log::info!(
            "GPU adapter: {} [{:?} / {:?}], present mode {:?}",
            exposed.info.name,
            exposed.info.device_type,
            exposed.info.backend,
            surface_config.present_mode
        );

        // SAFETY: plain object creation on the device just opened.
        let fence: A::Fence = unsafe { device.create_fence() }.map_err(gpu_err)?;
        let mut upload_encoder: A::CommandEncoder = unsafe {
            device.create_command_encoder(&hal::CommandEncoderDescriptor {
                label: Some("REDIXEL_UPLOAD_ENCODER"),
                queue: &queue,
            })
        }
        .map_err(gpu_err)?;

        let pipelines: Pipelines<A> = Pipelines::new(&device, surface_config.format)?;
        let frames: Vec<Frame<A>> = (0..FRAMES_IN_FLIGHT)
            .map(|_| Frame::new(&device, &queue, &pipelines.globals_layout))
            .collect::<Result<Vec<Frame<A>>, RedixelError>>()?;
        let depth: Depth<A> = Depth::new(&device, surface_config.extent)?;

        let mut last_submitted: hal::FenceValue = 0;
        let mut upload: Upload<'_, A> = Upload {
            device:         &device,
            queue:          &queue,
            fence:          &fence,
            last_submitted: &mut last_submitted,
            encoder:        &mut upload_encoder,
            layout:         &pipelines.texture_layout,
            sampler:        &pipelines.sampler,
        };
        // SAFETY: the present thread is not running yet, so the queue is ours.
        let textures: Textures<A> = unsafe {
            Textures::new(
                upload.texture("REDIXEL_TEXTURE_DEFAULT", 1, 1, &WHITE_PIXEL_RGBA)?,
                upload.texture(
                    "REDIXEL_TEXTURE_MISSING",
                    CHECKERBOARD_SIZE,
                    CHECKERBOARD_SIZE,
                    &checkerboard_rgba(CHECKERBOARD_SIZE, CHECKERBOARD_CELL),
                )?,
            )
        };

        Ok(Self {
            frames,
            current: 0,
            upload_encoder,
            textures: ManuallyDrop::new(textures),
            pipelines: ManuallyDrop::new(pipelines),
            depth: ManuallyDrop::new(depth),
            depth_fresh: true,
            fence: ManuallyDrop::new(fence),
            last_submitted,
            surface_config,
            surface_views: Vec::new(),
            presenter: Presenter::new(config.present_spin),
            surface: Some(Arc::new(surface)),
            queue: Arc::new(queue),
            device,
            adapter_info: exposed.info,
            max_texture: exposed.capabilities.limits.max_texture_dimension_2d,
            _adapter: exposed.adapter,
            instance,
            _window: window,
        })
    }

    /// Creates a texture from tightly packed RGBA8 texels, waiting for the
    /// upload to finish.
    fn upload(&mut self, label: &str, width: u32, height: u32, texels: &[u8]) -> Result<GpuTexture<A>, RedixelError> {
        self.join_present();

        let mut upload: Upload<'_, A> = Upload {
            device:         &self.device,
            queue:          &self.queue,
            fence:          &self.fence,
            last_submitted: &mut self.last_submitted,
            encoder:        &mut self.upload_encoder,
            layout:         &self.pipelines.texture_layout,
            sampler:        &self.pipelines.sampler,
        };
        // SAFETY: the present thread was just joined, so the queue is ours.
        unsafe { upload.texture(label, width, height, texels) }
    }

    /// Waits for the present in flight, if any. Called before every queue or
    /// swapchain operation outside `render`.
    fn join_present(&mut self) {
        if let Err(e) = self.presenter.join() {
            log::warn!("Deferred present failed: {e}");
        }
    }

    /// # Safety
    ///
    /// The queue is idle, so no submission still renders into the views.
    unsafe fn drop_surface_views(&mut self) {
        for (_, view) in self.surface_views.drain(..) {
            unsafe { self.device.destroy_texture_view(view) };
        }
    }

    fn record(
        &mut self,
        clear: Color,
        batch_2d: &mut Batch2d,
        geometry_3d: &Geometry,
        time: f32,
    ) -> Result<(), RedixelError> {
        let Self {
            frames,
            current,
            textures,
            pipelines,
            depth,
            depth_fresh,
            fence,
            last_submitted,
            surface_config,
            surface_views,
            presenter,
            surface,
            queue,
            device,
            ..
        } = self;

        let Some(surface) = surface.as_ref() else {
            return Ok(());
        };

        profile::phase("wait_fence");
        let frame: &mut Frame<A> = &mut frames[*current];

        // SAFETY: waiting for the slot's own fence value retires every command
        // buffer, view, uniform and stream the slot recorded last time, so all
        // of them may be reset, destroyed or overwritten from here on.
        let acquired: hal::AcquiredSurfaceTexture<A> = unsafe {
            device.wait(fence, frame.fence_value, None).map_err(gpu_err)?;
            frame.recycle(device);

            profile::phase("uniforms");
            let (uniforms_2d, uniforms_3d): (GlobalUniforms, GlobalUniforms) =
                frame_uniforms(surface_config.extent.width, surface_config.extent.height, time);
            write_mapped::<A>(device, &frame.globals_2d.buffer, bytemuck::bytes_of(&uniforms_2d))?;
            write_mapped::<A>(device, &frame.globals_3d.buffer, bytemuck::bytes_of(&uniforms_3d))?;

            profile::phase("buffer_upload");
            batch_2d.finish();
            frame
                .stream_3d
                .upload(device, &geometry_3d.vertices, &geometry_3d.indices, &[])?;
            frame
                .stream_2d
                .upload(device, &batch_2d.vertices, &batch_2d.indices, &batch_2d.quads)?;
            frame.encoder.begin_encoding(Some("REDIXEL_FRAME")).map_err(gpu_err)?;

            profile::phase("acquire");
            match presenter
                .join()
                .and_then(|()| surface.acquire_texture(Some(ACQUIRE_TIMEOUT), fence))
            {
                Ok(acquired) => acquired,
                Err(e) => {
                    frame.encoder.discard_encoding();
                    return Err(surface_error(e));
                }
            }
        };

        profile::phase("encode");
        let target: &A::Texture = acquired.texture.borrow();

        // SAFETY: the surface texture was just acquired and leaves its
        // undefined state before the pass clears it; the depth attachment gets
        // the same one-off transition after every (re)creation; everything
        // bound below belongs to this device and outlives the submission,
        // because the slot keeps it until its fence value passes.
        unsafe {
            let fresh_view: Option<A::TextureView> = match A::surface_image_key(target) {
                Some(key) => {
                    if !surface_views
                        .iter()
                        .any(|(cached, _): &(u64, A::TextureView)| *cached == key)
                    {
                        let view: A::TextureView = device
                            .create_texture_view(target, &surface_view(surface_config))
                            .map_err(gpu_err)?;
                        surface_views.push((key, view));
                    }
                    None
                }
                None => Some(
                    device
                        .create_texture_view(target, &surface_view(surface_config))
                        .map_err(gpu_err)?,
                ),
            };
            let view: &A::TextureView = match &fresh_view {
                Some(view) => view,
                None => {
                    let key: Option<u64> = A::surface_image_key(target);
                    surface_views
                        .iter()
                        .find(|(cached, _): &&(u64, A::TextureView)| Some(*cached) == key)
                        .map(|(_, view): &(u64, A::TextureView)| view)
                        .ok_or_else(|| gpu_err("swapchain view cache miss"))?
                }
            };

            frame.encoder.transition_textures(iter::once(hal::TextureBarrier {
                texture: target,
                range:   wgt::ImageSubresourceRange::default(),
                usage:   hal::StateTransition {
                    from: wgt::TextureUses::UNINITIALIZED,
                    to:   wgt::TextureUses::COLOR_TARGET,
                },
            }));
            if *depth_fresh {
                frame.encoder.transition_textures(iter::once(hal::TextureBarrier {
                    texture: &depth.texture,
                    range:   wgt::ImageSubresourceRange::default(),
                    usage:   hal::StateTransition {
                        from: wgt::TextureUses::UNINITIALIZED,
                        to:   wgt::TextureUses::DEPTH_STENCIL_WRITE,
                    },
                }));
                *depth_fresh = false;
            }

            frame
                .encoder
                .begin_render_pass(&hal::RenderPassDescriptor {
                    label:                    Some("REDIXEL_PASS"),
                    extent:                   surface_config.extent,
                    sample_count:             1,
                    color_attachments:        &[Some(hal::ColorAttachment {
                        target:         hal::Attachment {
                            view,
                            usage: wgt::TextureUses::COLOR_TARGET,
                        },
                        depth_slice:    None,
                        resolve_target: None,
                        ops:            hal::AttachmentOps::LOAD_CLEAR | hal::AttachmentOps::STORE,
                        clear_value:    clear.into(),
                    })],
                    depth_stencil_attachment: Some(hal::DepthStencilAttachment {
                        target:      hal::Attachment {
                            view:  &depth.view,
                            usage: wgt::TextureUses::DEPTH_STENCIL_WRITE,
                        },
                        depth_ops:   hal::AttachmentOps::LOAD_CLEAR | hal::AttachmentOps::STORE,
                        stencil_ops: hal::AttachmentOps::LOAD | hal::AttachmentOps::STORE,
                        clear_value: (1.0, 0),
                    }),
                    multiview_mask:           None,
                    timestamp_writes:         None,
                    occlusion_query_set:      None,
                })
                .map_err(gpu_err)?;

            profile::phase("draw");
            draw_3d(
                &mut frame.encoder,
                pipelines,
                textures,
                &frame.globals_3d.group,
                &frame.stream_3d,
                geometry_3d,
            );
            draw_2d(
                &mut frame.encoder,
                pipelines,
                textures,
                &frame.globals_2d.group,
                &frame.stream_2d,
                batch_2d,
            );

            frame.encoder.end_render_pass();
            frame.encoder.transition_textures(iter::once(hal::TextureBarrier {
                texture: target,
                range:   wgt::ImageSubresourceRange::default(),
                usage:   hal::StateTransition {
                    from: wgt::TextureUses::COLOR_TARGET,
                    to:   wgt::TextureUses::PRESENT,
                },
            }));

            profile::phase("submit");
            let commands: A::CommandBuffer = frame.encoder.end_encoding().map_err(gpu_err)?;
            *last_submitted += 1;
            let submitted: Result<(), hal::DeviceError> =
                queue.submit(&[&commands], &[&acquired.texture], (fence, *last_submitted));
            frame.used.push(commands);
            frame.used_views.extend(fresh_view);
            frame.fence_value = *last_submitted;
            *current = (*current + 1) % frames.len();
            submitted.map_err(gpu_err)?;
        }

        profile::phase("present");
        presenter.submit(PresentJob {
            queue:   Arc::clone(queue),
            surface: Arc::clone(surface),
            texture: acquired.texture,
        })
    }
}

impl<A: ApiExt> Backend for Gpu<A> {
    fn load_texture(&mut self, id: TextureId, bytes: &[u8]) -> Result<(), RedixelError> {
        if let Some(old) = self.textures.replace(id, None) {
            self.join_present();
            // SAFETY: the queue is idle, so no frame still samples the old texture.
            unsafe {
                let _ = self.queue.wait_for_idle();
                old.destroy(&self.device);
            }
        }

        let rgba: image::RgbaImage = decode_rgba(bytes, self.max_texture)?;
        let (width, height): (u32, u32) = rgba.dimensions();
        let texture: GpuTexture<A> = self.upload(&format!("REDIXEL_TEXTURE_{}", id.index()), width, height, &rgba)?;
        self.textures.replace(id, Some(texture));
        Ok(())
    }

    fn suspend(&mut self) {
        self.join_present();
        if let Some(surface) = self.surface.take() {
            // SAFETY: the queue is idle before the views are destroyed and the
            // surface unconfigured, and the present thread holds no reference.
            unsafe {
                let _ = self.queue.wait_for_idle();
                self.drop_surface_views();
                surface.unconfigure(&self.device);
            }
        }
    }

    fn resume(&mut self, window: &Arc<dyn Window>) -> Result<(), RedixelError> {
        if self.surface.is_none() {
            let surface: A::Surface = create_surface::<A>(&self.instance, window)?;
            // SAFETY: the stored configuration was resolved on this adapter; a
            // mismatch with the new surface is reported as an error.
            unsafe { surface.configure(&self.device, &self.surface_config) }.map_err(gpu_err)?;
            self.surface = Some(Arc::new(surface));
        }

        Ok(())
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        self.surface_config.extent.width = new_size.width;
        self.surface_config.extent.height = new_size.height;
        self.join_present();

        // SAFETY: the queue is idle, so neither the swapchain nor the old
        // depth attachment is in use.
        unsafe {
            let _ = self.queue.wait_for_idle();
            self.drop_surface_views();
            if let Some(surface) = &self.surface
                && let Err(e) = surface.configure(&self.device, &self.surface_config)
            {
                log::error!("Surface reconfiguration failed: {e}");
            }

            match Depth::new(&self.device, self.surface_config.extent) {
                Ok(depth) => {
                    ManuallyDrop::take(&mut self.depth).destroy(&self.device);
                    self.depth = ManuallyDrop::new(depth);
                    self.depth_fresh = true;
                }
                Err(e) => log::error!("Depth buffer recreation failed: {e}"),
            }
        }
    }

    fn surface_size(&self) -> (u32, u32) {
        (self.surface_config.extent.width, self.surface_config.extent.height)
    }

    fn adapter_info(&self) -> &wgt::AdapterInfo {
        &self.adapter_info
    }

    fn present_mode(&self) -> wgt::PresentMode {
        self.surface_config.present_mode
    }

    fn render(&mut self, input: FrameInput<'_>, time: f32) -> Result<(), RedixelError> {
        let FrameInput {
            clear,
            batch_2d,
            geometry_3d,
        } = input;
        let drawn: Result<(), RedixelError> = self.record(clear, batch_2d, geometry_3d, time);
        batch_2d.clear();
        geometry_3d.clear();
        drawn
    }
}

impl<A: ApiExt> Drop for Gpu<A> {
    fn drop(&mut self) {
        self.join_present();

        // SAFETY: the queue is idle, so nothing destroyed here is still in
        // use, and each `ManuallyDrop` field is taken exactly once.
        unsafe {
            let _ = self.queue.wait_for_idle();

            for frame in self.frames.drain(..) {
                frame.destroy(&self.device);
            }
            ManuallyDrop::take(&mut self.textures).destroy(&self.device);
            ManuallyDrop::take(&mut self.depth).destroy(&self.device);
            ManuallyDrop::take(&mut self.pipelines).destroy(&self.device);
            self.device.destroy_fence(ManuallyDrop::take(&mut self.fence));
            self.drop_surface_views();

            if let Some(surface) = self.surface.take() {
                surface.unconfigure(&self.device);
            }
        }
    }
}

fn surface_error(e: hal::SurfaceError) -> RedixelError {
    match e {
        hal::SurfaceError::Lost | hal::SurfaceError::Outdated => RedixelError::SurfaceNeedsReconfiguration,
        hal::SurfaceError::Timeout | hal::SurfaceError::Occluded => RedixelError::SurfaceIgnored,
        e => gpu_err(e),
    }
}

fn surface_view(config: &hal::SurfaceConfiguration) -> hal::TextureViewDescriptor<'static> {
    hal::TextureViewDescriptor {
        label:     Some("REDIXEL_SURFACE_VIEW"),
        format:    config.format,
        dimension: wgt::TextureViewDimension::D2,
        usage:     wgt::TextureUses::COLOR_TARGET,
        range:     wgt::ImageSubresourceRange::default(),
    }
}

/// Records one indexed draw per texture run of `geometry`.
///
/// # Safety
///
/// Called inside an active render pass, after this frame's 3D upload.
unsafe fn draw_3d<A: ApiExt>(
    encoder: &mut A::CommandEncoder,
    pipelines: &Pipelines<A>,
    textures: &Textures<A>,
    globals: &A::BindGroup,
    stream: &Stream<A>,
    geometry: &Geometry,
) {
    let (Some(vertex), Some(index)): (Option<&StreamBuffer<A>>, Option<&StreamBuffer<A>>) =
        (stream.vertex.as_ref(), stream.index.as_ref())
    else {
        return;
    };
    if geometry.indices.is_empty() {
        return;
    }

    unsafe {
        encoder.set_render_pipeline(&pipelines.triangles_3d);
        encoder.set_bind_group(&pipelines.layout, 0, globals, &[]);
        encoder.set_vertex_buffer(0, hal::BufferBinding::new_unchecked(vertex.buffer(), 0, None));
        encoder.set_index_buffer(
            hal::BufferBinding::new_unchecked(index.buffer(), 0, None),
            wgt::IndexFormat::Uint32,
        );
        for run in &geometry.runs {
            encoder.set_bind_group(&pipelines.layout, 1, textures.group(run.texture), &[]);
            encoder.draw_indexed(run.start, run.end - run.start, 0, 0, 1);
        }
    }
}

/// Records `batch` in submission order, switching between the instanced quad
/// pipeline and the indexed triangle pipeline at run boundaries. Both share
/// one layout, so group 0 stays bound across the switch.
///
/// # Safety
///
/// Called inside an active render pass, after this frame's 2D upload and
/// [`Batch2d::finish`].
unsafe fn draw_2d<A: ApiExt>(
    encoder: &mut A::CommandEncoder,
    pipelines: &Pipelines<A>,
    textures: &Textures<A>,
    globals: &A::BindGroup,
    stream: &Stream<A>,
    batch: &Batch2d,
) {
    if batch.runs.is_empty() {
        return;
    }

    let mut bound: Option<RunKind> = None;

    unsafe {
        encoder.set_bind_group(&pipelines.layout, 0, globals, &[]);
        for run in &batch.runs {
            if bound != Some(run.kind) {
                match (run.kind, &stream.instance, &stream.vertex, &stream.index) {
                    (RunKind::Quads, Some(instance), _, _) => {
                        encoder.set_render_pipeline(&pipelines.quads);
                        encoder.set_vertex_buffer(0, hal::BufferBinding::new_unchecked(instance.buffer(), 0, None));
                    }
                    (RunKind::Triangles, _, Some(vertex), Some(index)) => {
                        encoder.set_render_pipeline(&pipelines.triangles_2d);
                        encoder.set_vertex_buffer(0, hal::BufferBinding::new_unchecked(vertex.buffer(), 0, None));
                        encoder.set_index_buffer(
                            hal::BufferBinding::new_unchecked(index.buffer(), 0, None),
                            wgt::IndexFormat::Uint32,
                        );
                    }
                    _ => continue,
                }
                bound = Some(run.kind);
            }

            encoder.set_bind_group(&pipelines.layout, 1, textures.group(run.texture), &[]);
            match run.kind {
                RunKind::Quads => encoder.draw(0, 4, run.start, run.end - run.start),
                RunKind::Triangles => encoder.draw_indexed(run.start, run.end - run.start, 0, 0, 1),
            }
        }
    }
}

fn create_surface<A: hal::Api>(instance: &A::Instance, window: &Arc<dyn Window>) -> Result<A::Surface, RedixelError> {
    let display: raw_window_handle::RawDisplayHandle = window.display_handle().map_err(gpu_err)?.as_raw();
    // Windows refuses `window_handle` off the event-loop thread, and the
    // renderer is built on a background one; the `Arc` keeps the window alive.
    #[cfg(target_os = "windows")]
    let handle: raw_window_handle::RawWindowHandle = {
        use winit::platform::windows::WindowExtWindows as _;
        // SAFETY: the handle is only used to create the surface, while `Gpu`
        // holds the window alive.
        unsafe { window.window_handle_any_thread() }.map_err(gpu_err)?.as_raw()
    };
    #[cfg(not(target_os = "windows"))]
    let handle: raw_window_handle::RawWindowHandle = {
        use raw_window_handle::HasWindowHandle as _;
        window.window_handle().map_err(gpu_err)?.as_raw()
    };
    // SAFETY: `Gpu` holds the window's `Arc` and drops the surface before it.
    unsafe { instance.create_surface(display, handle) }.map_err(gpu_err)
}

/// Prefers a discrete GPU among the adapters able to present to `surface`.
fn pick_adapter<A: hal::Api>(
    instance: &A::Instance,
    surface: &A::Surface,
) -> Result<(hal::ExposedAdapter<A>, hal::SurfaceCapabilities), RedixelError> {
    // SAFETY: `surface` was created from this instance.
    let mut adapters: Vec<hal::ExposedAdapter<A>> = unsafe { instance.enumerate_adapters(Some(surface)) };
    adapters.sort_by_key(|a: &hal::ExposedAdapter<A>| a.info.device_type != wgt::DeviceType::DiscreteGpu);

    adapters
        .into_iter()
        .find_map(|exposed: hal::ExposedAdapter<A>| {
            // SAFETY: `surface` was created from the instance that enumerated this adapter.
            let caps: Option<hal::SurfaceCapabilities> = unsafe { exposed.adapter.surface_capabilities(surface) };
            caps.map(|caps: hal::SurfaceCapabilities| (exposed, caps))
        })
        .ok_or_else(|| gpu_err("no adapter can present to this window"))
}

/// An sRGB format in the sRGB colour space, the requested present mode or
/// else the surface's first one, opaque alpha when offered.
fn surface_config(
    caps: &hal::SurfaceCapabilities,
    size: PhysicalSize<u32>,
    requested: wgt::PresentMode,
) -> Result<hal::SurfaceConfiguration, RedixelError> {
    let srgb_space: Vec<wgt::TextureFormat> = caps
        .formats
        .iter()
        .filter(|f: &&wgt::SurfaceFormatCapabilities| f.color_spaces.contains(wgt::SurfaceColorSpaces::SRGB))
        .map(|f: &wgt::SurfaceFormatCapabilities| f.format)
        .collect();
    let format: wgt::TextureFormat = srgb_space
        .iter()
        .copied()
        .find(|f: &wgt::TextureFormat| f.is_srgb())
        .or_else(|| srgb_space.first().copied())
        .ok_or_else(|| gpu_err("surface offers no sRGB colour space"))?;

    let present_mode: wgt::PresentMode = if caps.present_modes.contains(&requested) {
        requested
    } else {
        caps.present_modes[0]
    };

    let composite_alpha_mode: wgt::CompositeAlphaMode =
        if caps.composite_alpha_modes.contains(&wgt::CompositeAlphaMode::Opaque) {
            wgt::CompositeAlphaMode::Opaque
        } else {
            caps.composite_alpha_modes[0]
        };

    Ok(hal::SurfaceConfiguration {
        maximum_frame_latency: FRAME_LATENCY
            .clamp(*caps.maximum_frame_latency.start(), *caps.maximum_frame_latency.end()),
        present_mode,
        composite_alpha_mode,
        format,
        color_space: wgt::SurfaceColorSpace::Srgb,
        extent: wgt::Extent3d {
            width:                 size.width.max(1),
            height:                size.height.max(1),
            depth_or_array_layers: 1,
        },
        usage: wgt::TextureUses::COLOR_TARGET,
        view_formats: Vec::new(),
    })
}
