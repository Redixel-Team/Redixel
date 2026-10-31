//! Networking transports for the Redixel engine: WebTransport (native and browser) and an in-process loopback, all behind `NetworkManager`.

mod frame;
mod inbound;
mod seq;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub mod config;
pub mod loopback;
#[cfg(not(target_arch = "wasm32"))]
pub mod webtransport;

pub use config::{CertSource, DEFAULT_PROTOCOL_ID, NetConfig, NetMode, build};
pub use loopback::LoopbackNetwork;
#[cfg(target_arch = "wasm32")]
pub use wasm::WasmWebTransportClient;
#[cfg(not(target_arch = "wasm32"))]
pub use webtransport::{WebTransportClient, WebTransportServer};
