use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(not(target_arch = "wasm32"))]
use winit::dpi::LogicalSize;

use winit::{
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    monitor::Fullscreen,
    window::{Window, WindowAttributes},
};

use redixel_core::RedixelError;

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
}

/// Owns the OS window and exposes a minimal surface for the rest of the engine.
///
/// Nothing outside this crate should hold a direct reference to the underlying
/// `winit::Window` — use the methods here instead.
#[derive(Debug)]
pub struct WindowManager {
    window: Arc<dyn Window>,
}

impl WindowManager {
    pub fn new(event_loop: &dyn ActiveEventLoop, config: &WindowConfig) -> Result<Self, RedixelError> {
        let attrs: WindowAttributes = Self::build_attributes(config)?;

        Ok(Self {
            window: Arc::from(event_loop.create_window(attrs)?),
        })
    }

    /// Returns a cloned `Arc` so the renderer can share ownership of the window.
    pub fn window_arc(&self) -> Arc<dyn Window> {
        self.window.clone()
    }

    pub fn surface_size(&self) -> PhysicalSize<u32> {
        self.window.surface_size()
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Requests a physical surface size. Some display systems apply it
    /// asynchronously and report the result in a later resize event.
    pub fn request_surface_size(&self, width: u32, height: u32) -> Option<PhysicalSize<u32>> {
        self.window
            .request_surface_size(PhysicalSize::new(width, height).into())
    }

    /// Switches between a decorated window and borderless fullscreen on the
    /// monitor that currently contains the window.
    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.window
            .set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
    }

    pub fn is_fullscreen(&self) -> bool {
        self.window.fullscreen().is_some()
    }

    /// Updates the title bar with the current FPS, and the network RTT in
    /// milliseconds when `rtt_ms` is `Some` (a connected client). No-op on WASM.
    #[allow(unused_variables)]
    pub fn set_title_stats(&self, fps: f64, rtt_ms: Option<f64>) {
        #[cfg(not(target_arch = "wasm32"))]
        match rtt_ms {
            Some(ms) => self.window.set_title(&format!("Redixel — {fps:.0} FPS — {ms:.0} ms")),
            None => self.window.set_title(&format!("Redixel — {fps:.0} FPS")),
        }
    }

    /// Returns `true` for events that the window manager should process.
    pub fn process_window_event(&self, event: &WindowEvent) -> bool {
        matches!(event, WindowEvent::Focused(..) | WindowEvent::ScaleFactorChanged { .. })
    }

    fn build_attributes(config: &WindowConfig) -> Result<WindowAttributes, RedixelError> {
        let attrs: WindowAttributes = WindowAttributes::default().with_title(config.title.clone());
        Self::apply_platform_attrs(attrs, config)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn apply_platform_attrs(
        mut attrs: WindowAttributes,
        config: &WindowConfig,
    ) -> Result<WindowAttributes, RedixelError> {
        attrs = attrs.with_surface_size(LogicalSize::new(config.width, config.height));

        if config.fullscreen {
            attrs = attrs.with_fullscreen(Some(winit::monitor::Fullscreen::Borderless(None)));
        }

        Ok(attrs)
    }

    #[cfg(target_arch = "wasm32")]
    fn set_css_property(style: &web_sys::CssStyleDeclaration, prop: &str, val: &str) -> Result<(), RedixelError> {
        style
            .set_property(prop, val)
            .map_err(|_| RedixelError::JsException("Failed to set CSS property.".into()))
    }

    #[cfg(target_arch = "wasm32")]
    fn apply_platform_attrs(attrs: WindowAttributes, config: &WindowConfig) -> Result<WindowAttributes, RedixelError> {
        use winit::platform::web::WindowAttributesWeb;

        let web_window: web_sys::Window =
            web_sys::window().ok_or(RedixelError::JsException("Global 'window' not found.".into()))?;

        let document: web_sys::Document = web_window
            .document()
            .ok_or(RedixelError::JsException("Global 'document' not found.".into()))?;

        let body: web_sys::HtmlElement = document
            .body()
            .ok_or(RedixelError::JsException("Global 'body' not found.".into()))?;

        let canvas: web_sys::HtmlCanvasElement = match document.get_element_by_id("redixel-canvas") {
            Some(elem) => elem
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .map_err(|_| RedixelError::JsException("Element is not a <canvas>.".into()))?,
            None => {
                log::info!("Injecting Redixel canvas into DOM...");

                let elem: web_sys::Element = document
                    .create_element("canvas")
                    .map_err(|_| RedixelError::JsException("Failed to create <canvas>.".into()))?;

                elem.set_id("redixel-canvas");

                body.append_child(&elem)
                    .map_err(|_| RedixelError::JsException("Failed to append canvas to body.".into()))?;

                elem.dyn_into::<web_sys::HtmlCanvasElement>().unwrap()
            }
        };

        let body_style: web_sys::CssStyleDeclaration = body.style();
        let canvas_style: web_sys::CssStyleDeclaration = canvas.style();

        Self::set_css_property(&body_style, "margin", "0")?;
        Self::set_css_property(&body_style, "padding", "0")?;
        Self::set_css_property(&body_style, "overflow", "hidden")?;

        Self::set_css_property(&body_style, "display", "flex")?;
        Self::set_css_property(&body_style, "justify-content", "center")?;
        Self::set_css_property(&body_style, "align-items", "center")?;
        Self::set_css_property(&body_style, "height", "100vh")?;

        if config.fullscreen {
            Self::set_css_property(&canvas_style, "width", "100vw")?;
            Self::set_css_property(&canvas_style, "height", "100vh")?;
        } else {
            Self::set_css_property(&canvas_style, "width", &format!("{}px", config.width))?;
            Self::set_css_property(&canvas_style, "height", &format!("{}px", config.height))?;
        }

        let web_attrs: WindowAttributesWeb = WindowAttributesWeb::default().with_canvas(Some(canvas));
        Ok(attrs.with_platform_attributes(Box::new(web_attrs)))
    }
}
