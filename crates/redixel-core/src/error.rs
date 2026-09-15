use thiserror::Error;
use winit::error::{EventLoopError, RequestError};

#[cfg(feature = "wgpu")]
use wgpu::{CreateSurfaceError, RequestAdapterError, RequestDeviceError};

#[cfg(all(target_os = "windows", feature = "wgpu"))]
use wgpu::rwh::HandleError;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum RedixelError {
    #[cfg(feature = "wgpu")]
    #[error("Failed to create rendering surface: {0}")]
    CreateSurface(#[from] CreateSurfaceError),

    #[cfg(all(target_os = "windows", feature = "wgpu"))]
    #[error("Failed to acquire raw window handle: {0}")]
    WindowHandle(#[from] HandleError),

    #[cfg(feature = "wgpu")]
    #[error("Failed to find a suitable graphics adapter: {0}")]
    RequestAdapter(#[from] RequestAdapterError),

    #[cfg(feature = "wgpu")]
    #[error("Failed to create graphics device: {0}")]
    RequestDevice(#[from] RequestDeviceError),

    #[error("Window system request failed: {0}")]
    WindowRequest(#[from] RequestError),

    #[error("Event loop fatal error: {0}")]
    EventLoop(#[from] EventLoopError),

    #[error("Surface needs reconfiguration")]
    SurfaceNeedsReconfiguration,

    #[error("Surface timeout or occluded")]
    SurfaceIgnored,

    #[error("Surface validation error")]
    SurfaceValidation,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config parse error: {0}")]
    Config(#[from] serde_json::Error),

    #[error("Texture decode error: {0}")]
    TextureDecode(String),

    #[error("Audio device error: {0}")]
    AudioDevice(String),

    #[error("Audio decode error: {0}")]
    AudioDecode(String),

    #[error("Logger initialization failed: {0}")]
    Logger(String),

    #[cfg(target_arch = "wasm32")]
    #[error("JavaScript exception: {0}")]
    JsException(String),

    #[error("Dummy error (test only)")]
    Dummy,
}

impl From<log::SetLoggerError> for RedixelError {
    fn from(e: log::SetLoggerError) -> Self {
        RedixelError::Logger(e.to_string())
    }
}

#[cfg(target_arch = "wasm32")]
impl From<RedixelError> for JsValue {
    fn from(e: RedixelError) -> JsValue {
        JsValue::from_str(&e.to_string())
    }
}
