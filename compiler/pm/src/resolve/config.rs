use std::{fs, path::PathBuf};

use scoop_manifest::package::RegistryUrl;

use crate::{
    PmError, PmResult,
    cache::{FileStamp, PackageCache, atomic_write},
};

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Config {
    #[serde(default)]
    registry: Registry,
}
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    default: Option<RegistryUrl>,
}

#[derive(Default)]
pub(super) struct LoadedConfig {
    pub registry: Option<RegistryUrl>,
    pub input: Option<(PathBuf, Option<FileStamp>)>,
}

pub(crate) fn path() -> Option<PathBuf> {
    let directory = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    };
    directory.map(|directory| directory.join("scoop/config.toml"))
}

pub(super) fn registry(cache: &PackageCache) -> PmResult<LoadedConfig> {
    let Some(path) = path() else {
        return Ok(LoadedConfig::default());
    };
    let Some(stamp) = FileStamp::optional(&path)? else {
        return Ok(LoadedConfig {
            registry: None,
            input: Some((path, None)),
        });
    };
    let record = cache.input_record("config-v1", &path, &stamp, &[])?;
    let config: Config = match fs::read(&record) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|error| PmError::new(error.to_string()))?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let text = fs::read_to_string(&path).map_err(|error| PmError::io(&path, error))?;
            let config: Config = toml::from_str(&text)
                .map_err(|error| PmError::new(format!("{}: {error}", path.display())))?;
            stamp.unchanged(&path)?;
            atomic_write(
                &record,
                &serde_json::to_vec(&config).map_err(|error| PmError::new(error.to_string()))?,
            )?;
            config
        }
        Err(error) => return Err(PmError::io(&record, error)),
    };
    Ok(LoadedConfig {
        registry: config.registry.default,
        input: Some((path, Some(stamp))),
    })
}
