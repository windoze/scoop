use std::fmt;
use std::path::{Path, PathBuf};

use scoop_lir::ValidatedLirTargetSelection;
use scoop_toolchain::TrustedCoreSlotLayoutV1;

mod artifact;
pub use artifact::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustedCoreSlotIoOperation {
    CanonicalizeSysroot,
    InspectSysroot,
}

impl fmt::Display for TrustedCoreSlotIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CanonicalizeSysroot => "canonicalize sysroot",
            Self::InspectSysroot => "inspect sysroot",
        })
    }
}

#[derive(Debug)]
pub struct TrustedCoreSlotError {
    path: PathBuf,
    kind: TrustedCoreSlotErrorKind,
}

impl TrustedCoreSlotError {
    fn new(path: PathBuf, kind: TrustedCoreSlotErrorKind) -> Self {
        Self { path, kind }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn kind(&self) -> &TrustedCoreSlotErrorKind {
        &self.kind
    }
}

#[derive(Debug)]
pub enum TrustedCoreSlotErrorKind {
    Io {
        operation: TrustedCoreSlotIoOperation,
        source: std::io::Error,
    },
    SysrootNotDirectory,
}

impl fmt::Display for TrustedCoreSlotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot resolve trusted core slot from {}: ",
            self.path.display()
        )?;
        match &self.kind {
            TrustedCoreSlotErrorKind::Io { operation, source } => {
                write!(formatter, "cannot {operation}: {source}")
            }
            TrustedCoreSlotErrorKind::SysrootNotDirectory => {
                formatter.write_str("resolved sysroot is not a directory")
            }
        }
    }
}

impl std::error::Error for TrustedCoreSlotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            TrustedCoreSlotErrorKind::Io { source, .. } => Some(source),
            TrustedCoreSlotErrorKind::SysrootNotDirectory => None,
        }
    }
}

pub fn resolve_trusted_core_slot(
    target: ValidatedLirTargetSelection,
) -> Result<TrustedCoreSlotLayoutV1, TrustedCoreSlotError> {
    resolve_trusted_core_slot_at(&configured_sysroot_root(), target)
}

pub(crate) fn configured_sysroot_root() -> PathBuf {
    match std::env::var_os("SCOOP_SYSROOT") {
        Some(path) => PathBuf::from(path),
        None => super::workspace_root().join("sysroot"),
    }
}

pub(crate) fn resolve_trusted_core_slot_at(
    sysroot: &Path,
    target: ValidatedLirTargetSelection,
) -> Result<TrustedCoreSlotLayoutV1, TrustedCoreSlotError> {
    let real_sysroot = std::fs::canonicalize(sysroot).map_err(|source| {
        TrustedCoreSlotError::new(
            sysroot.to_path_buf(),
            TrustedCoreSlotErrorKind::Io {
                operation: TrustedCoreSlotIoOperation::CanonicalizeSysroot,
                source,
            },
        )
    })?;
    let metadata = std::fs::metadata(&real_sysroot).map_err(|source| {
        TrustedCoreSlotError::new(
            real_sysroot.clone(),
            TrustedCoreSlotErrorKind::Io {
                operation: TrustedCoreSlotIoOperation::InspectSysroot,
                source,
            },
        )
    })?;
    if !metadata.is_dir() {
        return Err(TrustedCoreSlotError::new(
            real_sysroot,
            TrustedCoreSlotErrorKind::SysrootNotDirectory,
        ));
    }

    Ok(TrustedCoreSlotLayoutV1::new(&real_sysroot, target))
}

#[cfg(test)]
mod tests;
