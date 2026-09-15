use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
    sync::{OnceLock, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Error, Map, Value};

use wgpu::{Backends, PresentMode};

use redixel_core::RedixelError;

#[derive(Debug, Default)]
pub struct EngineSettings {
    data: Value,
    loaded: bool,
}

impl EngineSettings {
    /// Returns the global `EngineSettings` instance.
    pub fn global() -> &'static RwLock<EngineSettings> {
        static INSTANCE: OnceLock<RwLock<EngineSettings>> = OnceLock::new();
        INSTANCE.get_or_init(|| RwLock::new(EngineSettings::default()))
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
        self.loaded = true;
        Ok(())
    }

    /// Loads `config/config.json` into the global settings, logging a warning
    /// (and continuing with defaults) if the file is missing or malformed.
    pub fn load_config_json() {
        if let Err(e) = Self::global_write().load("config/config.json") {
            log::warn!("Failed to read config/config.json, using defaults. Error: {e}");
        }
    }

    /// Whether a [`load`](Self::load) has succeeded.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Writes the settings to a JSON file, keeping keys in the order they were
    /// read or first set.
    ///
    /// The file is replaced atomically: the JSON goes to a sibling `.tmp` file
    /// that is then renamed over `path`, so a failed write never leaves a
    /// truncated file behind.
    pub fn save(&self, path: &str) -> Result<(), RedixelError> {
        let temp: String = format!("{path}.tmp");

        let saved: Result<(), RedixelError> = self
            .write_to(&temp)
            .and_then(|()| fs::rename(&temp, path).map_err(RedixelError::from));

        if saved.is_err() {
            fs::remove_file(&temp).ok();
        }

        saved
    }

    fn write_to(&self, path: &str) -> Result<(), RedixelError> {
        let mut writer: BufWriter<File> = BufWriter::new(File::create(path)?);
        serde_json::to_writer_pretty(&mut writer, &self.data)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        Ok(())
    }

    /// Writes the global settings back to `config/config.json`, logging a
    /// warning if it cannot be written.
    ///
    /// Does nothing, besides logging, when the file never loaded — missing or
    /// malformed — so it is never replaced by one holding only the keys set
    /// since.
    pub fn save_config_json() {
        let settings: RwLockReadGuard<'static, EngineSettings> = Self::global_read();

        if !settings.is_loaded() {
            log::warn!("config/config.json never loaded, so settings changed this session were not saved.");
            return;
        }

        if let Err(e) = settings.save("config/config.json") {
            log::warn!("Failed to write config/config.json: {e}");
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

    /// Writes a nested value using the same dot-notation as
    /// [`get_path`](Self::get_path), creating intermediate objects as needed.
    ///
    /// A segment currently holding something other than an object is replaced
    /// with one. Does nothing, besides logging, if `value` cannot be
    /// serialised to JSON.
    pub fn set_path<T: Serialize>(&mut self, path: &str, value: T) {
        let value: Value = match serde_json::to_value(value) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("Failed to serialize value for settings key `{path}`: {e}");
                return;
            }
        };

        let (parents, leaf): (Option<&str>, &str) = match path.rsplit_once('.') {
            Some((parents, leaf)) => (Some(parents), leaf),
            None => (None, path),
        };

        let mut node: &mut Value = &mut self.data;
        for key in parents.into_iter().flat_map(|parents: &str| parents.split('.')) {
            node = as_object(node).entry(key).or_insert(Value::Object(Map::new()));
        }

        as_object(node).insert(leaf.to_string(), value);
    }
}

/// `node` as an object, replacing whatever it held with an empty object first
/// if it was anything else.
fn as_object(node: &mut Value) -> &mut Map<String, Value> {
    if !node.is_object() {
        *node = Value::Object(Map::new());
    }
    node.as_object_mut().expect("the node was just made an object")
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp_path(test_name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("redixel_engine_settings_{test_name}_{}.json", std::process::id()))
    }

    #[test]
    fn a_fresh_instance_falls_back_to_defaults_and_is_not_loaded() {
        let settings: EngineSettings = EngineSettings::default();

        assert_eq!(settings.get_path("window.width", 1280), 1280);
        assert!(!settings.is_loaded());
    }

    #[test]
    fn set_path_then_get_path_returns_the_new_value() {
        let mut settings: EngineSettings = EngineSettings::default();
        settings.set_path("audio.master_volume", 0.5_f64);

        assert_eq!(settings.get_path("audio.master_volume", 1.0_f64), 0.5);
    }

    #[test]
    fn set_path_creates_missing_intermediate_objects() {
        let mut settings: EngineSettings = EngineSettings::default();
        settings.set_path("a.b.c", 42);

        assert_eq!(settings.get_path("a.b.c", 0), 42);
    }

    #[test]
    fn set_path_replaces_a_non_object_segment_with_an_object() {
        let mut settings: EngineSettings = EngineSettings::default();
        settings.set_path("a", 5);
        settings.set_path("a.b", 7);

        assert_eq!(settings.get_path("a.b", 0), 7);
    }

    #[test]
    fn set_path_leaves_sibling_keys_alone() {
        let mut settings: EngineSettings = EngineSettings::default();
        settings.set_path("window.width", 1280);
        settings.set_path("window.height", 720);

        assert_eq!(settings.get_path("window.width", 0), 1280);
        assert_eq!(settings.get_path("window.height", 0), 720);
    }

    #[test]
    fn a_failed_load_leaves_the_settings_unloaded() {
        let mut settings: EngineSettings = EngineSettings::default();

        assert!(settings.load("/no/such/directory/config.json").is_err());
        assert!(!settings.is_loaded());
    }

    #[test]
    fn save_then_load_round_trips_and_keeps_the_key_order() {
        let path: PathBuf = temp_path("round_trip");
        let path_str: &str = path.to_str().expect("temp paths are UTF-8");
        fs::write(&path, "{\"window\": {\"width\": 1280}, \"app\": {\"name\": \"Redixel\"}}").expect("seed file");

        let mut written: EngineSettings = EngineSettings::default();
        written.load(path_str).expect("seed file loads");
        written.set_path("audio.master_volume", 0.5_f64);
        written.save(path_str).expect("save succeeds");

        let text: String = fs::read_to_string(&path).expect("saved file reads");
        let mut read_back: EngineSettings = EngineSettings::default();
        read_back.load(path_str).expect("saved file loads");
        fs::remove_file(&path).ok();

        assert!(read_back.is_loaded());
        assert_eq!(read_back.get_path("window.width", 0), 1280);
        assert_eq!(read_back.get_path("audio.master_volume", 0.0_f64), 0.5);
        assert!(
            text.find("window") < text.find("app") && text.find("app") < text.find("audio"),
            "keys must keep the order they were read and added in:\n{text}"
        );
        assert!(text.ends_with("}\n"), "the file must end with a newline");
    }

    #[test]
    fn save_leaves_no_temporary_file_behind() {
        let path: PathBuf = temp_path("no_temp");
        let settings: EngineSettings = EngineSettings::default();

        settings
            .save(path.to_str().expect("temp paths are UTF-8"))
            .expect("save succeeds");

        let temp: PathBuf = PathBuf::from(format!("{}.tmp", path.display()));
        let temp_exists: bool = temp.exists();
        fs::remove_file(&path).ok();

        assert!(!temp_exists);
    }

    #[test]
    fn save_to_an_unwritable_path_returns_err() {
        let settings: EngineSettings = EngineSettings::default();
        assert!(settings.save("/no/such/directory/config.json").is_err());
    }
}
