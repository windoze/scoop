use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use super::{CacheIoOperation, CachePathRole, CompileCacheStoreError};

pub(super) fn ensure_private_directory(
    path: &Path,
    role: CachePathRole,
) -> Result<(), CompileCacheStoreError> {
    match std::fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(source) => {
            return Err(io_error(CacheIoOperation::CreateDirectory, path, source));
        }
    }
    validate_directory(path, role)?;
    set_private_directory_permissions(path)
}

pub(super) fn validate_directory(
    path: &Path,
    role: CachePathRole,
) -> Result<(), CompileCacheStoreError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|source| io_error(CacheIoOperation::Inspect, path, source))?;
    if !metadata.file_type().is_dir() {
        return Err(CompileCacheStoreError::InvalidPathType {
            role,
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

pub(crate) fn open_lock_file(path: &Path) -> Result<File, CompileCacheStoreError> {
    match open_lock_file_create_new(path) {
        Ok(file) => Ok(file),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(path)
                .map_err(|source| io_error(CacheIoOperation::Inspect, path, source))?;
            if !metadata.file_type().is_file() {
                return Err(CompileCacheStoreError::InvalidPathType {
                    role: CachePathRole::LockFile,
                    path: path.to_path_buf(),
                });
            }
            open_lock_file_existing(path)
                .map_err(|source| io_error(CacheIoOperation::Open, path, source))
        }
        Err(source) => Err(io_error(CacheIoOperation::CreateFile, path, source)),
    }
}

#[cfg(unix)]
fn open_lock_file_create_new(path: &Path) -> std::io::Result<File> {
    use rustix::fs::{Mode, OFlags};
    rustix::fs::open(
        path,
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )
    .map(File::from)
    .map_err(std::io::Error::from)
}

#[cfg(not(unix))]
fn open_lock_file_create_new(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
}

#[cfg(unix)]
fn open_lock_file_existing(path: &Path) -> std::io::Result<File> {
    use rustix::fs::{Mode, OFlags};
    rustix::fs::open(
        path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(std::io::Error::from)
}

#[cfg(not(unix))]
fn open_lock_file_existing(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).write(true).open(path)
}

pub(super) fn validate_opened_regular_file(
    file: &File,
    path: &Path,
    role: CachePathRole,
) -> Result<(), CompileCacheStoreError> {
    let opened = file
        .metadata()
        .map_err(|source| io_error(CacheIoOperation::Inspect, path, source))?;
    let visible = std::fs::symlink_metadata(path)
        .map_err(|source| io_error(CacheIoOperation::Inspect, path, source))?;
    if !opened.is_file() || !visible.file_type().is_file() || !same_file_identity(&opened, &visible)
    {
        return Err(CompileCacheStoreError::InvalidPathType {
            role,
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn same_file_identity(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file_identity(_left: &std::fs::Metadata, _right: &std::fs::Metadata) -> bool {
    true
}

pub(super) fn write_cache_file(path: &Path, bytes: &[u8]) -> Result<(), CompileCacheStoreError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| io_error(CacheIoOperation::CreateFile, path, source))?;
    file.write_all(bytes)
        .map_err(|source| io_error(CacheIoOperation::Write, path, source))?;
    set_read_only_file_permissions(path)?;
    file.sync_all()
        .map_err(|source| io_error(CacheIoOperation::Sync, path, source))?;
    Ok(())
}

pub(crate) fn sync_directory(path: &Path) -> Result<(), CompileCacheStoreError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| io_error(CacheIoOperation::Sync, path, source))
}

#[cfg(unix)]
pub(super) fn atomic_rename_no_replace(
    source: &Path,
    destination: &Path,
) -> Result<AtomicRenameOutcome, CompileCacheStoreError> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};
    use rustix::io::Errno;
    match renameat_with(CWD, source, CWD, destination, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(AtomicRenameOutcome::Published),
        Err(Errno::EXIST) => Ok(AtomicRenameOutcome::AlreadyExists),
        Err(error) => Err(io_error(
            CacheIoOperation::Publish,
            destination,
            std::io::Error::from(error),
        )),
    }
}

#[cfg(not(unix))]
pub(super) fn atomic_rename_no_replace(
    _source: &Path,
    destination: &Path,
) -> Result<AtomicRenameOutcome, CompileCacheStoreError> {
    Err(CompileCacheStoreError::AtomicNoReplaceUnavailable(
        destination.to_path_buf(),
    ))
}

pub(super) enum AtomicRenameOutcome {
    Published,
    AlreadyExists,
}

#[cfg(unix)]
pub(super) fn set_private_directory_permissions(path: &Path) -> Result<(), CompileCacheStoreError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|source| io_error(CacheIoOperation::SetPermissions, path, source))
}

#[cfg(not(unix))]
pub(super) fn set_private_directory_permissions(path: &Path) -> Result<(), CompileCacheStoreError> {
    let mut permissions = std::fs::metadata(path)
        .map_err(|source| io_error(CacheIoOperation::Inspect, path, source))?
        .permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions)
        .map_err(|source| io_error(CacheIoOperation::SetPermissions, path, source))
}

#[cfg(unix)]
fn set_read_only_file_permissions(path: &Path) -> Result<(), CompileCacheStoreError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o400))
        .map_err(|source| io_error(CacheIoOperation::SetPermissions, path, source))
}

#[cfg(not(unix))]
fn set_read_only_file_permissions(path: &Path) -> Result<(), CompileCacheStoreError> {
    let mut permissions = std::fs::metadata(path)
        .map_err(|source| io_error(CacheIoOperation::Inspect, path, source))?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions)
        .map_err(|source| io_error(CacheIoOperation::SetPermissions, path, source))
}

pub(super) fn io_error(
    operation: CacheIoOperation,
    path: &Path,
    source: std::io::Error,
) -> CompileCacheStoreError {
    CompileCacheStoreError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}
