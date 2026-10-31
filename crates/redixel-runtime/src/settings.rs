use std::{
    fs::File,
    io::BufReader,
    sync::{OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use redixel_core::RedixelError;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Error, Value};
use wgpu::{Backends, PresentMode};

#[derive(Debug)]
pub struct EngineSettings {
    data: Value,
}

impl EngineSettings {
    /// Returns the global `EngineSettings` instance.
    pub fn global() -> &'static RwLock<EngineSettings> {
        static INSTANCE: OnceLock<RwLock<EngineSettings>> = OnceLock::new();
        INSTANCE.get_or_init(|| RwLock::new(EngineSettings { data: Value::Null }))
    }

    /// Acquires a shared read lock. Recovers gracefully from a poisoned lock.
    pub fn global_read() -> RwLockReadGuard<'static, EngineSettings> {
        Self::global()
            .read()
            .unwrap_or_else(|p: PoisonError<RwLockReadGuard<'_, EngineSettings>>| {
                log::warn!("EngineSettings read-lock was poisoned — recovering.");
                p.into_inner()
            })
    }

    /// Acquires an exclusive write lock. Recovers gracefully from a poisoned lock.
    pub fn global_write() -> RwLockWriteGuard<'static, EngineSettings> {
        Self::global()
            .write()
            .unwrap_or_else(|p: PoisonError<RwLockWriteGuard<'static, EngineSettings>>| {
                log::warn!("EngineSettings write-lock was poisoned — recovering.");
                p.into_inner()
            })
    }

    /// Loads settings from a JSON file.
    ///
    /// Returns `Err` if the file cannot be opened or JSON is malformed.
    /// The caller decides whether to hard-fail or continue with defaults.
    pub fn load(&mut self, path: &str) -> Result<(), RedixelError> {
        let file: File = File::open(path)?;
        self.data = serde_json::from_reader(BufReader::new(file))?;
        Ok(())
    }

    /// Loads `config/config.json` into the global settings, logging a warning
    /// (and continuing with defaults) if the file is missing or malformed.
    pub fn load_config_json() {
        if let Err(e) = Self::global_write().load("config/config.json") {
            log::warn!("Failed to read config/config.json, using defaults. Error: {e}");
        }
    }

    /// Retrieves a nested value using dot-notation (e.g. `"window.width"`).
    ///
    /// Returns `default` if any path segment is missing or the stored value
    /// cannot be deserialized into `T`.
    pub fn get_path<T: DeserializeOwned>(&self, path: &str, default: T) -> T {
        let mut node: &Value = &self.data;

        for key in path.split('.') {
            match node.get(key) {
                Some(v) => node = v,
                None => {
                    log::warn!("Settings key not found: `{path}`, using default.");
                    return default;
                }
            }
        }

        serde_json::from_value(node.clone()).unwrap_or_else(|_: Error| {
            log::warn!("Settings key `{path}` has an unexpected type, using default.");
            default
        })
    }
}

/// Raw integer code for `config.json → renderer.backend`.
#[derive(Debug, Deserialize)]
pub struct RawBackend(pub u32);

impl From<RawBackend> for Backends {
    fn from(raw: RawBackend) -> Self {
        match raw.0 {
            0 => Backends::all(),
            1 => Backends::VULKAN,
            2 => Backends::GL,
            3 => Backends::METAL,
            4 => Backends::DX12,
            5 => Backends::BROWSER_WEBGPU,
            6 => Backends::PRIMARY,
            7 => Backends::SECONDARY,
            other => {
                log::warn!("Unknown backend code {other}. defaulting to Backends::all().");
                Backends::all()
            }
        }
    }
}

/// Raw integer code for `config.json → renderer.present_mode`.
#[derive(Debug, Deserialize)]
pub struct RawPresentMode(pub u32);

impl From<RawPresentMode> for PresentMode {
    fn from(raw: RawPresentMode) -> Self {
        match raw.0 {
            0 => PresentMode::AutoVsync,
            1 => PresentMode::AutoNoVsync,
            2 => PresentMode::Fifo,
            3 => PresentMode::FifoRelaxed,
            4 => PresentMode::Immediate,
            5 => PresentMode::Mailbox,
            other => {
                log::warn!("Unknown present_mode code {other}. defaulting to AutoVsync.");
                PresentMode::AutoVsync
            }
        }
    }
}
