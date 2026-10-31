use std::sync::Arc;

use redixel_core::RedixelError;
use wgpu::{
    Adapter, AdapterInfo, BackendOptions, Backends, Device, ExperimentalFeatures, Extent3d, Features, Instance,
    InstanceDescriptor, InstanceFlags, MemoryBudgetThresholds, MemoryHints, PowerPreference, PresentMode, Queue,
    RequestAdapterOptions, Surface, SurfaceCapabilities, SurfaceColorSpace, SurfaceConfiguration, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor, Trace,
    wgt::{DeviceDescriptor, SurfaceConfiguration as WgtSurfaceConfiguration},
};
use winit::{dpi::PhysicalSize, window::Window};

use crate::renderer::RendererConfig;

pub(crate) const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

/// Owns the WGPU logical device, presentation surface, and submission queue.
///
/// This is a pure graphics-layer type with no knowledge of the windowing
/// system beyond the `Arc<dyn Window>` it receives at construction time.
/// All configurable behaviour is injected via [`RendererConfig`].
#[derive(Debug)]
pub(crate) struct GpuDevice {
    pub(crate) instance:   Instance,
    pub(crate) surface:    Option<Surface<'static>>,
    pub(crate) device:     Device,
    pub(crate) queue:      Queue,
    pub(crate) config:     SurfaceConfiguration,
    pub(crate) depth_view: TextureView,
}

impl GpuDevice {
    pub(crate) async fn new(window: Arc<dyn Window>, cfg: &RendererConfig) -> Result<Self, RedixelError> {
        let backends: Backends = Self::resolve_backends(cfg.backends).await;
        let instance: Instance = Self::create_instance(backends);
        let surface: Surface<'_> = Self::create_surface(&instance, &window)?;
        let adapter: Adapter = Self::request_adapter(&instance, &surface).await?;

        let (device, queue): (Device, Queue) = Self::request_device(&adapter).await?;
        let config: WgtSurfaceConfiguration<Vec<TextureFormat>> =
            Self::build_surface_config(&window, &surface, &adapter, cfg.present_mode);

        surface.configure(&device, &config);
        Self::log_adapter(&adapter);

        let depth_view: TextureView = Self::create_depth_view(&device, config.width, config.height);

        Ok(Self {
            device,
            queue,
            config,
            instance,
            surface: Some(surface),
            depth_view,
        })
    }

    /// Reconfigures the swap chain to match a new window size.
    /// No-ops for zero-area sizes (minimised window).
    ///
    /// The depth attachment is rebuilt alongside it: WGPU requires every
    /// attachment in a render pass to share one resolution, so a depth buffer
    /// left at the old size fails validation on the next draw.
    pub(crate) fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        self.config.width = new_size.width;
        self.config.height = new_size.height;

        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }

        self.depth_view = Self::create_depth_view(&self.device, self.config.width, self.config.height);
    }

    /// Drops the surface when the application is suspended.
    /// This is necessary for platforms like Android to release graphics resources.
    pub(crate) fn suspend(&mut self) {
        self.surface = None;
    }

    /// Re-initializes the surface after the application is resumed.
    /// Recreates the surface and configures it with the existing device settings.
    pub(crate) fn resume(&mut self, window: &Arc<dyn Window>) -> Result<(), RedixelError> {
        if self.surface.is_none() {
            let surface: Surface = Self::create_surface(&self.instance, window)?;
            surface.configure(&self.device, &self.config);
            self.surface = Some(surface);
        }

        Ok(())
    }

    /// Narrows the requested backend set to what the target can actually serve.
    ///
    /// Native targets take the set as given: WGPU enumerates every compiled-in
    /// backend at adapter-request time and picks across all of them.
    #[cfg(not(target_arch = "wasm32"))]
    async fn resolve_backends(requested: Backends) -> Backends {
        requested
    }

    /// Drops `BROWSER_WEBGPU` when the browser exposes `navigator.gpu` but has
    /// no adapter behind it.
    ///
    /// `Instance::new` commits to the WebGPU backend on the mere presence of
    /// `navigator.gpu` and never falls back afterwards, so a browser with
    /// WebGPU disabled, blocklisted, or unbacked by a usable GPU would never
    /// reach the WebGL2 path. Worse, WGPU 30 reads `requestAdapter()`'s `null`
    /// through `JsOption`, which only recognises `undefined` as empty, so the
    /// null adapter is accepted and the first property access on it throws an
    /// unrecoverable JS `TypeError`. Probing here keeps that null inside JS and
    /// leaves `Backends::GL` as the only candidate, routing the instance
    /// through `wgpu-core`.
    #[cfg(target_arch = "wasm32")]
    async fn resolve_backends(requested: Backends) -> Backends {
        if !requested.contains(Backends::BROWSER_WEBGPU) || Self::webgpu_adapter_available().await {
            return requested;
        }

        log::warn!("No WebGPU adapter available. Falling back to WebGL2.");
        requested.difference(Backends::BROWSER_WEBGPU)
    }

    /// Resolves `navigator.gpu.requestAdapter()` and reports whether it yielded
    /// an actual adapter.
    ///
    /// Reached through `Reflect` rather than `web-sys` on purpose: the typed
    /// WebGPU bindings sit behind the `web_sys_unstable_apis` cfg, and this
    /// probe must not add that constraint to every downstream build. Any
    /// missing property, thrown call, or rejected promise counts as "no
    /// adapter" — all of them mean the WebGPU backend cannot serve this page.
    #[cfg(target_arch = "wasm32")]
    async fn webgpu_adapter_available() -> bool {
        use js_sys::{Function, Promise, Reflect};
        use wasm_bindgen::{JsCast, JsValue};
        use wasm_bindgen_futures::JsFuture;

        fn property(target: &JsValue, key: &str) -> Option<JsValue> {
            Reflect::get(target, &JsValue::from_str(key))
                .ok()
                .filter(|value: &JsValue| !value.is_undefined() && !value.is_null())
        }

        let global: JsValue = js_sys::global().into();
        let Some(navigator) = property(&global, "navigator") else {
            return false;
        };

        let Some(gpu) = property(&navigator, "gpu") else {
            return false;
        };

        let Some(request) =
            property(&gpu, "requestAdapter").and_then(|value: JsValue| value.dyn_into::<Function>().ok())
        else {
            return false;
        };

        let Some(promise) = request
            .call0(&gpu)
            .ok()
            .and_then(|value: JsValue| value.dyn_into::<Promise>().ok())
        else {
            return false;
        };

        JsFuture::from(promise)
            .await
            .is_ok_and(|adapter: JsValue| !adapter.is_undefined() && !adapter.is_null())
    }

    fn create_instance(backends: Backends) -> Instance {
        Instance::new(InstanceDescriptor {
            backends,
            display: None,
            flags: InstanceFlags::default(),
            memory_budget_thresholds: MemoryBudgetThresholds::default(),
            backend_options: BackendOptions::default(),
        })
    }

    fn create_surface(instance: &Instance, window: &Arc<dyn Window>) -> Result<Surface<'static>, RedixelError> {
        #[cfg(target_os = "windows")]
        {
            // On Windows, `raw_window_handle()` enforces thread identity and will
            // return `HandleError::Unavailable` when called from outside the event-
            // loop thread. We initialise on a background thread, so we use
            // `window_handle_any_thread` to bypass the guard safely — the `Arc`
            // guarantees the window outlives this call.
            use wgpu::{SurfaceTargetUnsafe, rwh::HasDisplayHandle};
            use winit::platform::windows::WindowExtWindows;

            unsafe {
                instance
                    .create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
                        raw_display_handle: Some(window.display_handle()?.as_raw()),
                        raw_window_handle:  window.window_handle_any_thread()?.as_raw(),
                    })
                    .map_err(RedixelError::from)
            }
        }

        #[cfg(not(target_os = "windows"))]
        instance.create_surface(window.clone()).map_err(RedixelError::from)
    }

    /// Reports which GPU actually backs this session.
    ///
    /// Worth logging unconditionally: a software rasteriser (llvmpipe,
    /// lavapipe) is selected silently when no hardware adapter is present, and
    /// any timing measured against it says nothing about real hardware. Name
    /// and driver strings arrive blank under WebGPU — browsers withhold them
    /// to limit fingerprinting — so blanks are substituted or dropped instead
    /// of logging empty text.
    fn log_adapter(adapter: &Adapter) {
        let info: AdapterInfo = adapter.get_info();

        let name: &str = match info.name.trim() {
            "" => "unidentified adapter",
            name => name,
        };

        let driver: String = format!("{} {}", info.driver.trim(), info.driver_info.trim())
            .trim()
            .to_string();

        if driver.is_empty() {
            log::info!("GPU adapter: {name} [{:?} / {:?}]", info.device_type, info.backend);
        } else {
            log::info!(
                "GPU adapter: {name} [{:?} / {:?}] driver: {driver}",
                info.device_type,
                info.backend
            );
        }
    }

    async fn request_adapter(instance: &Instance, surface: &Surface<'static>) -> Result<Adapter, RedixelError> {
        instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(surface),
                force_fallback_adapter: false,
                ..Default::default()
            })
            .await
            .map_err(RedixelError::from)
    }

    async fn request_device(adapter: &Adapter) -> Result<(Device, Queue), RedixelError> {
        adapter
            .request_device(&DeviceDescriptor {
                label:                 Some("REDIXEL_DEVICE"),
                required_features:     Features::empty(),
                required_limits:       adapter.limits(),
                memory_hints:          MemoryHints::Performance,
                trace:                 Trace::Off,
                experimental_features: ExperimentalFeatures::default(),
            })
            .await
            .map_err(RedixelError::from)
    }

    /// Creates a `Depth32Float` texture sized to match the colour attachment
    /// and returns the view the render pass binds. The view keeps the texture
    /// alive, and nothing samples or copies the depth buffer.
    fn create_depth_view(device: &Device, width: u32, height: u32) -> TextureView {
        let texture: Texture = device.create_texture(&TextureDescriptor {
            label:           Some("REDIXEL_DEPTH_TEXTURE"),
            size:            Extent3d {
                width:                 width.max(1),
                height:                height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count:    1,
            dimension:       TextureDimension::D2,
            format:          DEPTH_FORMAT,
            usage:           TextureUsages::RENDER_ATTACHMENT,
            view_formats:    &[],
        });

        texture.create_view(&TextureViewDescriptor::default())
    }

    fn build_surface_config(
        window: &Arc<dyn Window>,
        surface: &Surface,
        adapter: &Adapter,
        desired_present_mode: PresentMode,
    ) -> SurfaceConfiguration {
        let size: PhysicalSize<u32> = window.surface_size();
        let caps: SurfaceCapabilities = surface.get_capabilities(adapter);

        let format: TextureFormat = caps
            .formats
            .iter()
            .copied()
            .find(|f: &TextureFormat| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let present_mode: PresentMode = caps
            .present_modes
            .iter()
            .copied()
            .find(|&m: &PresentMode| m == desired_present_mode)
            .unwrap_or(caps.present_modes[0]);

        SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![format],
            desired_maximum_frame_latency: 2,
            color_space: SurfaceColorSpace::Auto,
        }
    }
}
