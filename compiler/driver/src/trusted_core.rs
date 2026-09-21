use std::fmt;
use std::path::{Path, PathBuf};

use scoop_identity::ConeCoordinate;
use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::{CompositeIdentityAbiFingerprint, IdentityAbiDescriptor};
use scoop_toolchain::TrustedCoreSlotLayoutV1;
use scoop_wire::HashError;

mod artifact;
pub use artifact::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedCoreArtifactInput {
    path: PathBuf,
    expected_coordinate: ConeCoordinate,
    target: ValidatedLirTargetSelection,
    toolchain_compatibility: CompositeIdentityAbiFingerprint,
}

impl TrustedCoreArtifactInput {
    pub fn new(
        path: &Path,
        target: ValidatedLirTargetSelection,
    ) -> Result<Self, TrustedCoreArtifactInputError> {
        Ok(Self {
            path: canonical_regular_file(path)?,
            expected_coordinate: ConeCoordinate::reserved_core(),
            target,
            toolchain_compatibility: IdentityAbiDescriptor::current()
                .and_then(IdentityAbiDescriptor::fingerprint)
                .map_err(TrustedCoreArtifactInputError::ToolchainCompatibility)?,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn expected_coordinate(&self) -> &ConeCoordinate {
        &self.expected_coordinate
    }

    pub const fn target(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub const fn toolchain_compatibility(&self) -> CompositeIdentityAbiFingerprint {
        self.toolchain_compatibility
    }

    #[cfg(test)]
    pub(crate) fn for_test(path: PathBuf) -> Self {
        Self {
            path,
            expected_coordinate: ConeCoordinate::reserved_core(),
            target: ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            toolchain_compatibility: IdentityAbiDescriptor::current()
                .and_then(IdentityAbiDescriptor::fingerprint)
                .unwrap(),
        }
    }
}

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

#[derive(Debug)]
pub enum TrustedCoreArtifactInputError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    NotRegularFile(PathBuf),
    ToolchainCompatibility(HashError),
}

impl fmt::Display for TrustedCoreArtifactInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "cannot resolve core artifact {}: {source}",
                    path.display()
                )
            }
            Self::NotRegularFile(path) => write!(
                formatter,
                "trusted core artifact {} is not a regular file",
                path.display()
            ),
            Self::ToolchainCompatibility(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for TrustedCoreArtifactInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::NotRegularFile(_) => None,
            Self::ToolchainCompatibility(error) => Some(error),
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

fn canonical_regular_file(path: &Path) -> Result<PathBuf, TrustedCoreArtifactInputError> {
    let canonical =
        std::fs::canonicalize(path).map_err(|source| TrustedCoreArtifactInputError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let metadata =
        std::fs::metadata(&canonical).map_err(|source| TrustedCoreArtifactInputError::Io {
            path: canonical.clone(),
            source,
        })?;
    if !metadata.is_file() {
        return Err(TrustedCoreArtifactInputError::NotRegularFile(canonical));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests;
