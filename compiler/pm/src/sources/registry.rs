//! Sparse registry metadata and shared immutable source packages.

use std::{
    fs,
    path::{Path, PathBuf},
};

use scoop_identity::{ConeCoordinate, RequestedConeKind};
use scoop_manifest::{
    DependencyCoordinateKey, VersionRequirement,
    package::{PackageDocument, RegistryUrl},
};
use serde::{Deserialize, Serialize};

use crate::{
    PmError, PmResult,
    cache::{PackageCache, atomic_write},
};

mod archive;
mod fetch;
mod http;
mod index;
mod manifest;

pub(crate) use fetch::cached;
pub use fetch::{CachedPackage, fetch, fetch_locked};
pub use index::load_index;
pub use manifest::published_manifest;

pub struct RegistryClient<'a> {
    cache: &'a PackageCache,
    http: ureq::Agent,
}

impl<'a> RegistryClient<'a> {
    pub fn new(cache: &'a PackageCache) -> Self {
        let http = ureq::Agent::config_builder()
            .timeout_connect(Some(std::time::Duration::from_secs(30)))
            .timeout_per_read(Some(std::time::Duration::from_secs(30)))
            .build()
            .into();
        Self { cache, http }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryIndex {
    pub format: u32,
    pub group: String,
    pub name: String,
    pub versions: Vec<RegistryVersion>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryVersion {
    pub version: String,
    #[serde(with = "scoop_manifest::package::format::Kind")]
    pub kind: RequestedConeKind,
    pub dependencies: Vec<RegistryDependency>,
    pub archive: String,
    pub sha256: String,
    pub yanked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryDependency {
    pub key: DependencyCoordinateKey,
    pub version: VersionRequirement,
    pub source: RegistrySource,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum RegistrySource {
    Registry {
        registry: RegistryUrl,
    },
    Git {
        git: String,
        commit: String,
        subdir: String,
    },
    Sysroot,
}

pub(super) fn archive_url(registry: &RegistryUrl, archive: &str) -> PmResult<String> {
    let url = url::Url::parse(registry.as_str())
        .and_then(|base| base.join(archive))
        .map_err(|error| PmError::new(format!("invalid registry archive URL: {error}")))?;
    if archive.is_empty()
        || !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(PmError::new(
            "archive must be an HTTP(S) URL without embedded credentials or fragment",
        ));
    }
    Ok(url.into())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> PmResult<Option<T>> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(PmError::io(path, error)),
    };
    serde_json::from_reader(std::io::BufReader::new(file))
        .map(Some)
        .map_err(|error| PmError::new(format!("{}: {error}", path.display())))
}

fn write_json(path: &Path, value: &impl Serialize) -> PmResult<()> {
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|error| PmError::new(error.to_string()))?;
    atomic_write(path, &bytes)
}

#[cfg(test)]
mod tests;
