//! Shared package locations and ordinary atomic publication.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use scoop_manifest::package::RegistryUrl;

use crate::{PmError, PmResult};

#[derive(Clone, Debug)]
pub struct PackageCache {
    root: PathBuf,
}

impl PackageCache {
    pub fn resolve(cwd: &Path, explicit: Option<&Path>) -> PmResult<Self> {
        let root = match explicit
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("SCOOP_PACKAGE_CACHE_DIR").map(PathBuf::from))
        {
            Some(path) => cwd.join(path),
            None => home()?.join(".scoop/packages"),
        };
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn registry(&self, registry: &RegistryUrl) -> PathBuf {
        self.root
            .join("registry")
            .join(scoop_wire::sha256(registry.as_str().as_bytes()).to_string())
    }

    pub fn git(&self, repository: &str) -> PathBuf {
        self.root
            .join("git")
            .join(scoop_wire::sha256(repository.as_bytes()).to_string())
    }

    /// Holding the returned file owns the lock; dropping it releases the lock.
    pub(crate) fn lock(&self, namespace: &str, key: &[u8]) -> PmResult<File> {
        let directory = self.root.join("records/locks").join(namespace);
        fs::create_dir_all(&directory).map_err(|error| PmError::io(&directory, error))?;
        let path = directory.join(format!("{}.lock", scoop_wire::sha256(key)));
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|error| PmError::io(&path, error))?;
        fs4::FileExt::lock(&file).map_err(|error| PmError::io(&path, error))?;
        Ok(file)
    }
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> PmResult<()> {
    let directory = path
        .parent()
        .ok_or_else(|| PmError::new("output path has no parent"))?;
    fs::create_dir_all(directory).map_err(|error| PmError::io(directory, error))?;
    let mut output =
        tempfile::NamedTempFile::new_in(directory).map_err(|error| PmError::io(path, error))?;
    output
        .write_all(bytes)
        .map_err(|error| PmError::io(path, error))?;
    output.flush().map_err(|error| PmError::io(path, error))?;
    output
        .persist(path)
        .map_err(|error| PmError::io(path, error.error))?;
    Ok(())
}

fn home() -> PmResult<PathBuf> {
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| PmError::new("cannot locate the home directory; use --package-cache-dir"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_identity_separates_sources_and_shares_equivalent_urls() {
        let cache = PackageCache::resolve(Path::new("/work"), Some(Path::new("cache"))).unwrap();
        let first: RegistryUrl = "https://EXAMPLE.org:443/packages".parse().unwrap();
        let same: RegistryUrl = "https://example.org/packages/".parse().unwrap();
        let other: RegistryUrl = "https://other.example.org/packages/".parse().unwrap();
        assert_eq!(cache.root(), Path::new("/work/cache"));
        assert_eq!(cache.registry(&first), cache.registry(&same));
        assert_ne!(cache.registry(&first), cache.registry(&other));
        assert_ne!(
            cache.git("https://example.org/repo.git"),
            cache.git("ssh://example.org/repo.git")
        );
    }
}
