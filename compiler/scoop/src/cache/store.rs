use std::ffi::OsString;
use std::fs::File;
use std::path::{Path, PathBuf};

use fs4::FileExt;
use scoop_wire::{DecodeLimits, encode};

use super::{
    COMPILE_CACHE_NAMESPACE, CacheReceiptV1, ConeCompileCacheKeyV1, decode_cache_receipt_v1,
};
use crate::{ArtifactCacheRoot, ImmutableInputSnapshot};

mod error;
mod io;

pub use error::*;
use io::*;

const LOCK_DIRECTORY: &str = ".locks";
const STAGING_DIRECTORY: &str = ".staging";
const ARTIFACT_FILE_NAME: &str = "artifact.slib";
const RECEIPT_FILE_NAME: &str = "receipt.cbor";

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
        limits: DecodeLimits,
    ) -> Result<CompileCacheLookupV1, CompileCacheStoreError> {
        self.validate_lock(lock, None)?;
        lookup_entry(&self.entry_path(lock.key), lock.key, limits)
    }

    pub fn publish(
        &self,
        lock: &CompileCacheKeyLockV1,
        artifact: &ImmutableInputSnapshot,
        receipt: &CacheReceiptV1,
        limits: DecodeLimits,
    ) -> Result<CompileCachePublishV1, CompileCacheStoreError> {
        self.validate_lock(lock, Some(CompileCacheLockModeV1::Exclusive))?;
        if receipt.body().cache_key() != lock.key {
            return Err(CompileCacheStoreError::ReceiptKeyMismatch {
                expected: lock.key,
                actual: receipt.body().cache_key(),
            });
        }
        let receipt_bytes = encode(receipt).map_err(CompileCacheStoreError::ReceiptEncode)?;
        let receipt_limit = limits.owned_bytes;
        let receipt_length = u64::try_from(receipt_bytes.len())
            .map_err(|_| CompileCacheStoreError::LengthOverflow)?;
        if receipt_length > receipt_limit {
            return Err(CompileCacheStoreError::ReceiptTooLarge {
                limit: receipt_limit,
                observed: receipt_length,
            });
        }
        let staging = self.namespace.join(STAGING_DIRECTORY);
        let candidate = tempfile::Builder::new()
            .prefix("entry-")
            .tempdir_in(&staging)
            .map_err(|source| io_error(CacheIoOperation::CreateDirectory, &staging, source))?;
        set_private_directory_permissions(candidate.path())?;
        write_cache_file(
            &candidate.path().join(ARTIFACT_FILE_NAME),
            artifact.as_bytes(),
        )?;
        write_cache_file(&candidate.path().join(RECEIPT_FILE_NAME), &receipt_bytes)?;
        sync_directory(candidate.path())?;

        let verified = match lookup_entry(candidate.path(), lock.key, limits)? {
            CompileCacheLookupV1::Miss => {
                return Err(CompileCacheStoreError::CandidateVerificationMissing);
            }
            CompileCacheLookupV1::Hit(entry) => entry,
        };
        if verified.artifact().digest() != artifact.digest()
            || verified.artifact().as_bytes() != artifact.as_bytes()
            || verified.receipt() != receipt
        {
            return Err(CompileCacheStoreError::CandidateVerificationMismatch);
        }

        let destination = self.entry_path(lock.key);
        match atomic_rename_no_replace(candidate.path(), &destination)? {
            AtomicRenameOutcome::Published => {
                let _kept_path = candidate.keep();
                sync_directory(&self.namespace)?;
                match lookup_entry(&destination, lock.key, limits)? {
                    CompileCacheLookupV1::Hit(entry) => Ok(CompileCachePublishV1::Published(entry)),
                    CompileCacheLookupV1::Miss => Err(
                        CompileCacheStoreError::PublishedEntryDisappeared(destination),
                    ),
                }
            }
            AtomicRenameOutcome::AlreadyExists => {
                let winner = match lookup_entry(&destination, lock.key, limits)? {
                    CompileCacheLookupV1::Hit(entry) => entry,
                    CompileCacheLookupV1::Miss => {
                        return Err(CompileCacheStoreError::PublishedEntryDisappeared(
                            destination,
                        ));
                    }
                };
                if winner.artifact().digest() == artifact.digest()
                    && winner.artifact().as_bytes() == artifact.as_bytes()
                    && winner.receipt() == receipt
                {
                    Ok(CompileCachePublishV1::ExistingEquivalent(winner))
                } else {
                    Err(CompileCacheStoreError::NondeterministicProduction(
                        Box::new(CacheNondeterminismV1 {
                            existing_artifact: winner.artifact().digest(),
                            produced_artifact: artifact.digest(),
                            existing_receipt: winner.receipt().fingerprint(),
                            produced_receipt: receipt.fingerprint(),
                        }),
                    ))
                }
            }
        }
    }

    fn acquire(
        &self,
        key: ConeCompileCacheKeyV1,
        mode: CompileCacheLockModeV1,
    ) -> Result<CompileCacheKeyLockV1, CompileCacheStoreError> {
        self.ensure_layout()?;
        let path = self
            .namespace
            .join(LOCK_DIRECTORY)
            .join(format!("{key}.lock"));
        let file = open_lock_file(&path)?;
        match mode {
            CompileCacheLockModeV1::Shared => {
                FileExt::lock_shared(&file)
                    .map_err(|source| io_error(CacheIoOperation::Lock, &path, source))?;
            }
            CompileCacheLockModeV1::Exclusive => {
                FileExt::lock(&file)
                    .map_err(|source| io_error(CacheIoOperation::Lock, &path, source))?;
            }
        }
        validate_opened_regular_file(&file, &path, CachePathRole::LockFile)?;
        Ok(CompileCacheKeyLockV1 {
            namespace: self.namespace.clone(),
            key,
            mode,
            file,
            path,
        })
    }

    fn ensure_layout(&self) -> Result<(), CompileCacheStoreError> {
        let root = self
            .namespace
            .parent()
            .ok_or_else(|| CompileCacheStoreError::InvalidNamespace(self.namespace.clone()))?;
        std::fs::create_dir_all(root)
            .map_err(|source| io_error(CacheIoOperation::CreateDirectory, root, source))?;
        validate_directory(root, CachePathRole::CacheRoot)?;
        ensure_private_directory(&self.namespace, CachePathRole::Namespace)?;
        ensure_private_directory(
            &self.namespace.join(LOCK_DIRECTORY),
            CachePathRole::LockDirectory,
        )?;
        ensure_private_directory(
            &self.namespace.join(STAGING_DIRECTORY),
            CachePathRole::StagingDirectory,
        )
    }

    fn validate_lock(
        &self,
        lock: &CompileCacheKeyLockV1,
        required_mode: Option<CompileCacheLockModeV1>,
    ) -> Result<(), CompileCacheStoreError> {
        if lock.namespace != self.namespace || required_mode.is_some_and(|mode| lock.mode != mode) {
            return Err(CompileCacheStoreError::WrongLock);
        }
        validate_opened_regular_file(&lock.file, &lock.path, CachePathRole::LockFile)
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileCacheLookupV1 {
    Miss,
    Hit(Box<RawCompileCacheEntryV1>),
}

/// A physically stable cache pair. It deliberately grants no Compile or Link
/// artifact authority; callers must reopen both purpose views and validate the
/// receipt against the current plan before treating it as a cache hit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawCompileCacheEntryV1 {
    key: ConeCompileCacheKeyV1,
    entry_path: PathBuf,
    artifact: ImmutableInputSnapshot,
    receipt: CacheReceiptV1,
}

impl RawCompileCacheEntryV1 {
    pub const fn key(&self) -> ConeCompileCacheKeyV1 {
        self.key
    }

    pub fn entry_path(&self) -> &Path {
        &self.entry_path
    }

    pub const fn artifact(&self) -> &ImmutableInputSnapshot {
        &self.artifact
    }

    pub const fn receipt(&self) -> &CacheReceiptV1 {
        &self.receipt
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileCachePublishV1 {
    Published(Box<RawCompileCacheEntryV1>),
    ExistingEquivalent(Box<RawCompileCacheEntryV1>),
}

fn lookup_entry(
    entry_path: &Path,
    key: ConeCompileCacheKeyV1,
    limits: DecodeLimits,
) -> Result<CompileCacheLookupV1, CompileCacheStoreError> {
    let metadata = match std::fs::symlink_metadata(entry_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CompileCacheLookupV1::Miss);
        }
        Err(source) => {
            return Err(io_error(CacheIoOperation::Inspect, entry_path, source));
        }
    };
    if !metadata.file_type().is_dir() {
        return Err(CompileCacheStoreError::InvalidPathType {
            role: CachePathRole::EntryDirectory,
            path: entry_path.to_path_buf(),
        });
    }
    validate_entry_directory(entry_path)?;
    let artifact_path = entry_path.join(ARTIFACT_FILE_NAME);
    let receipt_path = entry_path.join(RECEIPT_FILE_NAME);
    let receipt_snapshot =
        ImmutableInputSnapshot::capture_no_follow(&receipt_path, limits.owned_bytes).map_err(
            |source| CompileCacheStoreError::Snapshot {
                role: CachePathRole::ReceiptFile,
                source,
            },
        )?;
    let (receipt, _) = decode_cache_receipt_v1(receipt_snapshot.as_bytes(), limits)
        .map_err(|source| CompileCacheStoreError::ReceiptDecode(Box::new(source)))?;
    if receipt.body().cache_key() != key {
        return Err(CompileCacheStoreError::ReceiptKeyMismatch {
            expected: key,
            actual: receipt.body().cache_key(),
        });
    }
    let artifact = ImmutableInputSnapshot::capture_no_follow(&artifact_path, limits.owned_bytes)
        .map_err(|source| CompileCacheStoreError::Snapshot {
            role: CachePathRole::ArtifactFile,
            source,
        })?;
    Ok(CompileCacheLookupV1::Hit(Box::new(
        RawCompileCacheEntryV1 {
            key,
            entry_path: entry_path.to_path_buf(),
            artifact,
            receipt,
        },
    )))
}

fn validate_entry_directory(path: &Path) -> Result<(), CompileCacheStoreError> {
    let mut names = std::fs::read_dir(path)
        .map_err(|source| io_error(CacheIoOperation::ListDirectory, path, source))?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name())
                .map_err(|source| io_error(CacheIoOperation::ListDirectory, path, source))
        })
        .collect::<Result<Vec<OsString>, _>>()?;
    names.sort();
    let expected = [
        OsString::from(ARTIFACT_FILE_NAME),
        OsString::from(RECEIPT_FILE_NAME),
    ];
    if names != expected {
        return Err(CompileCacheStoreError::UnexpectedEntryContents {
            path: path.to_path_buf(),
            names,
        });
    }
    for (name, role) in [
        (ARTIFACT_FILE_NAME, CachePathRole::ArtifactFile),
        (RECEIPT_FILE_NAME, CachePathRole::ReceiptFile),
    ] {
        let child = path.join(name);
        let metadata = std::fs::symlink_metadata(&child)
            .map_err(|source| io_error(CacheIoOperation::Inspect, &child, source))?;
        if !metadata.file_type().is_file() {
            return Err(CompileCacheStoreError::InvalidPathType { role, path: child });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
