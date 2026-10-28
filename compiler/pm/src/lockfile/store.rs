use std::{
    collections::BTreeSet,
    fs::{self, File},
    path::{Path, PathBuf},
};

use super::*;
use crate::{
    PmError, PmResult,
    cache::{PackageCache, atomic_write},
};

impl Lockfile {
    pub fn read(path: &Path) -> PmResult<Option<Self>> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(PmError::io(path, error)),
        };
        let lock: Self = toml::from_str(&text)
            .map_err(|error| PmError::new(format!("{}: {error}", path.display())))?;
        lock.validate()
            .map_err(|error| PmError::new(format!("{}: {error}", path.display())))?;
        Ok(Some(lock))
    }

    pub fn resolution(
        &self,
        root: &str,
        purpose: Purpose,
        target: &str,
    ) -> Option<&LockedResolution> {
        self.resolutions.iter().find(|resolution| {
            resolution.root == root && resolution.purpose == purpose && resolution.target == target
        })
    }
}

/// Serialize updates to one lock file. The caller rechecks manifest inputs
/// before publishing while this transaction still owns the file lock.
pub struct LockTransaction {
    path: PathBuf,
    current: Lockfile,
    _guard: File,
}

impl LockTransaction {
    pub fn open(path: &Path, cache: &PackageCache) -> PmResult<Self> {
        let directory = path
            .parent()
            .ok_or_else(|| PmError::new("lockfile has no parent directory"))?;
        fs::create_dir_all(directory).map_err(|error| PmError::io(directory, error))?;
        let directory =
            fs::canonicalize(directory).map_err(|error| PmError::io(directory, error))?;
        let path = directory.join(
            path.file_name()
                .ok_or_else(|| PmError::new("lockfile has no filename"))?,
        );
        let guard = cache.lock("lockfiles", path.as_os_str().as_encoded_bytes())?;
        let current = Lockfile::read(&path)?.unwrap_or_default();
        Ok(Self {
            path,
            current,
            _guard: guard,
        })
    }

    pub fn current(&self) -> &Lockfile {
        &self.current
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn publish(
        mut self,
        resolutions: Vec<LockedResolution>,
        members: &BTreeSet<String>,
    ) -> PmResult<Lockfile> {
        let original = self.current.clone();
        self.current
            .resolutions
            .retain(|resolution| members.contains(&resolution.root));
        for mut resolution in resolutions {
            resolution.validate()?;
            resolution.canonicalize();
            self.current.resolutions.retain(|existing| {
                (
                    existing.root.as_str(),
                    existing.purpose,
                    existing.target.as_str(),
                ) != (
                    resolution.root.as_str(),
                    resolution.purpose,
                    resolution.target.as_str(),
                )
            });
            self.current.resolutions.push(resolution);
        }
        self.current.resolutions.sort_by(|left, right| {
            (&left.root, left.purpose, &left.target).cmp(&(
                &right.root,
                right.purpose,
                &right.target,
            ))
        });
        if self.current != original {
            let text = toml::to_string_pretty(&self.current)
                .map_err(|error| PmError::new(error.to_string()))?;
            atomic_write(&self.path, text.as_bytes())?;
        }
        Ok(self.current)
    }
}
