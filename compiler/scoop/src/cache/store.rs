use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use fs4::FileExt;
use scoop_wire::{Decoder, WireDecode};

use super::{COMPILE_CACHE_NAMESPACE, CompileBuildRecordV1, ConeCompileCacheKeyV1};
use crate::ArtifactCacheRoot;

mod error;
mod io;

pub use error::*;
pub(crate) use io::open_build_lock;

const ARTIFACT_FILE: &str = "artifact.slib";
const RECORD_FILE: &str = "record.cbor";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileCacheStoreV1 {
    namespace: PathBuf,
}

impl CompileCacheStoreV1 {
    pub fn new(root: &ArtifactCacheRoot) -> Self {
        Self {
            namespace: root.as_path().join(COMPILE_CACHE_NAMESPACE),
        }
    }

    pub fn namespace_path(&self) -> &Path {
        &self.namespace
    }

    pub fn entry_path(&self, key: ConeCompileCacheKeyV1) -> PathBuf {
        self.namespace.join(key.to_string())
    }

    pub fn acquire_shared(
        &self,
        key: ConeCompileCacheKeyV1,
    ) -> Result<CompileCacheKeyLockV1, CompileCacheStoreError> {
        self.acquire(key, CompileCacheLockModeV1::Shared)
    }

    pub fn acquire_exclusive(
        &self,
        key: ConeCompileCacheKeyV1,
    ) -> Result<CompileCacheKeyLockV1, CompileCacheStoreError> {
        self.acquire(key, CompileCacheLockModeV1::Exclusive)
    }

    pub fn lookup(
        &self,
        lock: &CompileCacheKeyLockV1,
    ) -> Result<CompileCacheLookupV1, CompileCacheStoreError> {
        self.check_lock(lock, false)?;
        let entry = self.entry_path(lock.key);
        let path = entry.join(RECORD_FILE);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CompileCacheLookupV1::Miss);
            }
            Err(source) => {
                return Err(CompileCacheStoreError::io(
                    CacheIoOperation::Read,
                    path,
                    source,
                ));
            }
        };
        let mut decoder = Decoder::new(&bytes).map_err(CompileCacheStoreError::Decode)?;
        let record =
            CompileBuildRecordV1::decode(&mut decoder).map_err(CompileCacheStoreError::Decode)?;
        decoder.finish().map_err(CompileCacheStoreError::Decode)?;
        let artifact = entry.join(ARTIFACT_FILE);
        if !artifact.try_exists().map_err(|source| {
            CompileCacheStoreError::io(CacheIoOperation::Inspect, &artifact, source)
        })? {
            return Ok(CompileCacheLookupV1::Miss);
        }
        Ok(CompileCacheLookupV1::Hit(Box::new(CompileCacheEntryV1 {
            artifact,
            record,
        })))
    }

    pub fn publish(
        &self,
        lock: &CompileCacheKeyLockV1,
        output: &Path,
        record: CompileBuildRecordV1,
    ) -> Result<CompileCacheEntryV1, CompileCacheStoreError> {
        self.check_lock(lock, true)?;
        let entry = self.entry_path(lock.key);
        std::fs::create_dir_all(&entry).map_err(|source| {
            CompileCacheStoreError::io(CacheIoOperation::CreateDirectory, &entry, source)
        })?;
        let bytes = scoop_wire::encode(&record).map_err(CompileCacheStoreError::Encode)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&entry).map_err(|source| {
            CompileCacheStoreError::io(CacheIoOperation::Write, &entry, source)
        })?;
        temporary.write_all(&bytes).map_err(|source| {
            CompileCacheStoreError::io(CacheIoOperation::Write, temporary.path(), source)
        })?;
        let artifact = entry.join(ARTIFACT_FILE);
        std::fs::rename(output, &artifact).map_err(|source| {
            CompileCacheStoreError::io(CacheIoOperation::Publish, &artifact, source)
        })?;
        let path = entry.join(RECORD_FILE);
        temporary.persist(&path).map_err(|error| {
            CompileCacheStoreError::io(CacheIoOperation::Publish, path, error.error)
        })?;
        Ok(CompileCacheEntryV1 { artifact, record })
    }

    fn acquire(
        &self,
        key: ConeCompileCacheKeyV1,
        mode: CompileCacheLockModeV1,
    ) -> Result<CompileCacheKeyLockV1, CompileCacheStoreError> {
        let directory = self.namespace.join(".locks");
        std::fs::create_dir_all(&directory).map_err(|source| {
            CompileCacheStoreError::io(CacheIoOperation::CreateDirectory, &directory, source)
        })?;
        let path = directory.join(format!("{key}.lock"));
        let file = open_build_lock(&path)
            .map_err(|source| CompileCacheStoreError::io(CacheIoOperation::Open, &path, source))?;
        match mode {
            CompileCacheLockModeV1::Shared => FileExt::lock_shared(&file),
            CompileCacheLockModeV1::Exclusive => FileExt::lock(&file),
        }
        .map_err(|source| CompileCacheStoreError::io(CacheIoOperation::Lock, &path, source))?;
        Ok(CompileCacheKeyLockV1 {
            namespace: self.namespace.clone(),
            key,
            mode,
            file,
            path,
        })
    }

    fn check_lock(
        &self,
        lock: &CompileCacheKeyLockV1,
        exclusive: bool,
    ) -> Result<(), CompileCacheStoreError> {
        if lock.namespace != self.namespace
            || (exclusive && lock.mode != CompileCacheLockModeV1::Exclusive)
        {
            return Err(CompileCacheStoreError::WrongLock);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileCacheLockModeV1 {
    Shared,
    Exclusive,
}

#[derive(Debug)]
pub struct CompileCacheKeyLockV1 {
    namespace: PathBuf,
    key: ConeCompileCacheKeyV1,
    mode: CompileCacheLockModeV1,
    file: File,
    path: PathBuf,
}

impl CompileCacheKeyLockV1 {
    pub const fn key(&self) -> ConeCompileCacheKeyV1 {
        self.key
    }
    pub const fn mode(&self) -> CompileCacheLockModeV1 {
        self.mode
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CompileCacheKeyLockV1 {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[derive(Debug)]
pub enum CompileCacheLookupV1 {
    Miss,
    Hit(Box<CompileCacheEntryV1>),
}

#[derive(Debug)]
pub struct CompileCacheEntryV1 {
    pub artifact: PathBuf,
    pub record: CompileBuildRecordV1,
}

#[cfg(test)]
mod tests;
