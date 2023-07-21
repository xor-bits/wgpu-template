use std::{
    fs::{self, File},
    io::{Read, Write},
    ops::{Deref, DerefMut},
    sync::Arc,
};

use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use toml_edit::{Document, Entry, Item, TableLike, Value};
use wgpu::{Backends, PowerPreference};

//

pub static PROJECT_DIRS: Lazy<Option<ProjectDirs>> =
    Lazy::new(|| ProjectDirs::from("org", "xorbits", env!("CARGO_PKG_NAME")));

//

#[derive(Debug, Default, Clone)]
pub struct GlobalSettings {
    inner: SettingsInner,

    document: Option<Document>,
    // modified: Option<SystemTime>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SettingsInner {
    pub window: WindowSettings,
    pub graphics: GraphicsSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowSettings {
    pub resolution: (u32, u32),
    pub title: Arc<str>,
    pub force_wayland: bool,
    pub force_x11: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphicsSettings {
    pub allowed_backends: GraphicsBackends,
    pub gpu_preference: GpuPreference,
    pub force_software_rendering: bool,
    pub vsync: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphicsBackends {
    pub vulkan: bool,
    pub metal: bool,
    pub dx12: bool,
    pub webgpu: bool,

    pub gl: bool,
    pub dx11: bool,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub enum GpuPreference {
    #[default]
    HighPerformance,
    LowPower,
}

//

impl GlobalSettings {
    /// load the config from the config file (if found)
    ///
    /// or use the default configuration
    pub fn load() -> Self {
        match Self::try_load() {
            Ok(v) => v,
            Err(err) => {
                tracing::error!("Failed to load settings: {err}");
                Self::default()
            }
        }
    }

    pub fn try_load() -> Result<Self> {
        let mut file = Self::config_file()?;

        const DEFAULT: &str = include_str!("./settings.toml");

        let document: Document = if file.metadata()?.len() == 0 {
            file.write_all(DEFAULT.as_bytes())?;

            DEFAULT
                .parse()
                .map_err(|err| anyhow!("default config is invalid, this is a bug:\n{err}"))?
        } else {
            let mut buf = String::new();
            file.read_to_string(&mut buf)?;

            buf.parse()
                .map_err(|err| anyhow!("config is invalid:\n{err}"))?
        };

        /* file.flush()?;

        let modified = file.metadata().ok().and_then(|meta| meta.modified().ok()); */

        let mut inner: SettingsInner = toml_edit::de::from_document(document.clone())?;

        if inner.window.force_wayland && inner.window.force_x11 {
            tracing::error!("Both wayland and x11 were forced, ignoring both");
            inner.window.force_wayland = false;
            inner.window.force_x11 = false;
        }

        // let repaired_doc = toml_edit::ser::to_document(&inner)?;
        // Self::merge_document(document.as_table_mut(), repaired_doc.as_table());

        Ok(Self {
            document: Some(document),
            inner,
            // modified,
        })
    }

    pub fn autosave(&self) {
        if let Some(document) = self.document.as_ref() {
            self.save(document)
        }
    }

    pub fn save(&self, document: &Document) {
        if let Err(err) = self.try_save(document) {
            tracing::error!("Failed to load settings: {err}");
        }
    }

    pub fn try_save(&self, document: &Document) -> Result<()> {
        let mut file = Self::config_file()?;
        file.set_len(0)?;

        let contents = document.to_string();
        file.write_all(contents.as_bytes())?;

        Ok(())
    }

    /* fn get_new_if_modified(&self, file: &File) -> Option<Document> {
        let (Some(modified), Some(file_modified)) = (
            self.modified,
            file.metadata().ok().and_then(|meta| meta.modified().ok()),
        ) else {
            return None
        };

        if modified > file_modified {
            return None;
        }

        let mut buf = String::new();
        file.read_to_string(&mut buf).ok()?;

        buf.parse()?;

        Ok(())
    } */

    pub fn merge_document(original: &mut impl TableLike, new: &impl TableLike) {
        for (key, value) in new.iter() {
            if key.starts_with("_old_") {
                continue;
            }

            match original.entry(key) {
                Entry::Occupied(mut entry) => {
                    let entry = entry.get_mut();

                    match (entry, value) {
                        (Item::Table(entry), Item::Table(value)) => {
                            Self::merge_document(entry, value);
                            continue;
                        }
                        (
                            Item::Value(Value::InlineTable(entry)),
                            Item::Value(Value::InlineTable(value)),
                        ) => {
                            Self::merge_document(entry, value);
                            continue;
                        }
                        (Item::Table(entry), Item::Value(Value::InlineTable(value))) => {
                            Self::merge_document(entry, value);
                            continue;
                        }
                        (Item::Value(Value::InlineTable(entry)), Item::Table(value)) => {
                            Self::merge_document(entry, value);
                            continue;
                        }
                        (Item::Value(a), Item::Value(b)) if a.type_name() == b.type_name() => {
                            continue;
                        }
                        (entry, value) => {
                            tracing::error!("other: {entry:?}\n\n\n{value:?}");
                            let mut value = value.clone();
                            core::mem::swap(entry, &mut value);
                            original.insert(&format!("_old_{key}"), value);
                        }
                    };
                }
                Entry::Vacant(entry) => {
                    entry.insert(value.clone());
                }
            }
        }
    }

    pub fn config_file() -> Result<File> {
        let dirs = PROJECT_DIRS
            .as_ref()
            .ok_or_else(|| anyhow!("Could not get project dirs"))?;

        fs::create_dir_all(dirs.config_dir())?;

        let config = dirs.config_dir().join("settings.toml");
        Ok(fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(config)?)
    }
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            resolution: (1280, 720),
            title: "WGPU Template".into(),
            force_wayland: false,
            force_x11: false,
        }
    }
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            allowed_backends: <_>::default(),
            gpu_preference: <_>::default(),
            force_software_rendering: false,
            vsync: true,
        }
    }
}

impl Default for GraphicsBackends {
    fn default() -> Self {
        Self {
            vulkan: true,
            metal: true,
            dx12: true,
            webgpu: true,

            gl: false,
            dx11: false,
        }
    }
}

impl GraphicsBackends {
    pub fn to_backends(self) -> Backends {
        let mut backends = Backends::empty();

        backends.set(Backends::VULKAN, self.vulkan);
        backends.set(Backends::GL, self.gl);
        backends.set(Backends::METAL, self.metal);
        backends.set(Backends::DX12, self.dx12);
        backends.set(Backends::DX11, self.dx11);
        backends.set(Backends::BROWSER_WEBGPU, self.webgpu);

        backends
    }
}

impl GpuPreference {
    pub fn to_power_preference(self) -> PowerPreference {
        match self {
            GpuPreference::HighPerformance => PowerPreference::HighPerformance,
            GpuPreference::LowPower => PowerPreference::LowPower,
        }
    }
}

impl Deref for GlobalSettings {
    type Target = SettingsInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for GlobalSettings {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
