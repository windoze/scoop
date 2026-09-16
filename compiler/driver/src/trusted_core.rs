use std::fmt;
use std::path::{Path, PathBuf};

use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirTargetSelection;
use scoop_manifest::{LoadedConeManifest, ManifestRootError, ManifestRootLocator};
use scoop_slib::{CompositeIdentityAbiFingerprint, IdentityAbiDescriptor};
use scoop_toolchain::TrustedCoreSlotLayoutV1;
use scoop_wire::HashError;

mod artifact;
pub use artifact::*;

#[derive(Debug)]
pub struct TrustedCoreSourceSlot {
    manifest: LoadedConeManifest,
}

impl TrustedCoreSourceSlot {
    pub const fn manifest(&self) -> &LoadedConeManifest {
        &self.manifest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedCoreArtifactSlot {
    path: PathBuf,
    expected_coordinate: ConeCoordinate,
    target: ValidatedLirTargetSelection,
    toolchain_compatibility: CompositeIdentityAbiFingerprint,
}

impl TrustedCoreArtifactSlot {
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
}

#[derive(Debug)]
pub struct TrustedCoreSlot {
    source: TrustedCoreSourceSlot,
    artifact: TrustedCoreArtifactSlot,
}

impl TrustedCoreSlot {
    pub const fn source(&self) -> &TrustedCoreSourceSlot {
        &self.source
    }

    pub const fn artifact(&self) -> &TrustedCoreArtifactSlot {
        &self.artifact
    }

    pub fn into_bootstrap_parts(self) -> (TrustedCoreBootstrapInput, TrustedCoreArtifactSlot) {
        let authority = CoreBootstrapAuthority {
            source_root: self.source.manifest.real_root().to_path_buf(),
            artifact_path: self.artifact.path.clone(),
            expected_identity: ConeIdentity::CORE,
            target: self.artifact.target,
            toolchain_compatibility: self.artifact.toolchain_compatibility,
        };
        (
            TrustedCoreBootstrapInput {
                source_slot: self.source,
                authority,
            },
            self.artifact,
        )
    }

    pub fn existing_artifact_input(
        &self,
    ) -> Result<TrustedCoreArtifactInput, TrustedCoreArtifactInputError> {
        self.existing_artifact_input_at(self.artifact.path())
    }

    pub fn existing_artifact_input_at(
        &self,
        requested: &Path,
    ) -> Result<TrustedCoreArtifactInput, TrustedCoreArtifactInputError> {
        let configured = canonical_regular_file(self.artifact.path())?;
        let requested = canonical_regular_file(requested)?;
        if configured != requested {
            return Err(TrustedCoreArtifactInputError::WrongSlot {
                configured,
                requested,
            });
        }
        Ok(TrustedCoreArtifactInput {
            path: configured,
            expected_coordinate: self.artifact.expected_coordinate.clone(),
            target: self.artifact.target,
            toolchain_compatibility: self.artifact.toolchain_compatibility,
        })
    }
}

#[derive(Debug)]
pub struct TrustedCoreBootstrapInput {
    source_slot: TrustedCoreSourceSlot,
    authority: CoreBootstrapAuthority,
}

impl TrustedCoreBootstrapInput {
    pub const fn source_slot(&self) -> &TrustedCoreSourceSlot {
        &self.source_slot
    }

    pub const fn authority(&self) -> &CoreBootstrapAuthority {
        &self.authority
    }

    pub fn into_parts(self) -> (TrustedCoreSourceSlot, CoreBootstrapAuthority) {
        (self.source_slot, self.authority)
    }
}

#[derive(Debug)]
pub struct CoreBootstrapAuthority {
    source_root: PathBuf,
    artifact_path: PathBuf,
    expected_identity: ConeIdentity,
    target: ValidatedLirTargetSelection,
    toolchain_compatibility: CompositeIdentityAbiFingerprint,
}

impl CoreBootstrapAuthority {
    pub fn source_root(&self) -> &Path {
        &self.source_root
    }

    pub fn artifact_path(&self) -> &Path {
        &self.artifact_path
    }

    pub const fn expected_identity(&self) -> ConeIdentity {
        self.expected_identity
    }

    pub const fn target(&self) -> ValidatedLirTargetSelection {
        self.target
    }

    pub const fn toolchain_compatibility(&self) -> CompositeIdentityAbiFingerprint {
        self.toolchain_compatibility
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedCoreArtifactInput {
    path: PathBuf,
    expected_coordinate: ConeCoordinate,
    target: ValidatedLirTargetSelection,
    toolchain_compatibility: CompositeIdentityAbiFingerprint,
}

impl TrustedCoreArtifactInput {
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
    SourceManifest(ManifestRootError),
    ToolchainCompatibility(HashError),
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
            TrustedCoreSlotErrorKind::SourceManifest(error) => error.fmt(formatter),
            TrustedCoreSlotErrorKind::ToolchainCompatibility(error) => {
                write!(formatter, "cannot derive toolchain compatibility: {error}")
            }
        }
    }
}

impl std::error::Error for TrustedCoreSlotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            TrustedCoreSlotErrorKind::Io { source, .. } => Some(source),
            TrustedCoreSlotErrorKind::SourceManifest(error) => Some(error),
            TrustedCoreSlotErrorKind::ToolchainCompatibility(error) => Some(error),
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
    WrongSlot {
        configured: PathBuf,
        requested: PathBuf,
    },
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
            Self::WrongSlot {
                configured,
                requested,
            } => write!(
                formatter,
                "core artifact {} is not the configured trusted slot {}",
                requested.display(),
                configured.display()
            ),
        }
    }
}

impl std::error::Error for TrustedCoreArtifactInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::NotRegularFile(_) | Self::WrongSlot { .. } => None,
        }
    }
}

pub fn resolve_trusted_core_slot(
    target: ValidatedLirTargetSelection,
) -> Result<TrustedCoreSlot, TrustedCoreSlotError> {
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
) -> Result<TrustedCoreSlot, TrustedCoreSlotError> {
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

    let layout = TrustedCoreSlotLayoutV1::new(&real_sysroot, target);
    let source_path = layout.source_root().to_path_buf();
    let source = scoop_manifest::load_trusted_core_manifest(&ManifestRootLocator::cone_directory(
        &source_path,
    ))
    .map_err(|error| {
        TrustedCoreSlotError::new(source_path, TrustedCoreSlotErrorKind::SourceManifest(error))
    })?;
    let toolchain_compatibility = IdentityAbiDescriptor::current()
        .and_then(IdentityAbiDescriptor::fingerprint)
        .map_err(|error| {
            TrustedCoreSlotError::new(
                real_sysroot.clone(),
                TrustedCoreSlotErrorKind::ToolchainCompatibility(error),
            )
        })?;
    let artifact_path = layout.artifact().to_path_buf();

    Ok(TrustedCoreSlot {
        source: TrustedCoreSourceSlot { manifest: source },
        artifact: TrustedCoreArtifactSlot {
            path: artifact_path,
            expected_coordinate: ConeCoordinate::reserved_core(),
            target,
            toolchain_compatibility,
        },
    })
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
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new() -> Self {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "scoop-trusted-core-slot-{}-{serial}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(std::fs::canonicalize(path).unwrap())
        }

        fn write_core_manifest(&self, coordinate: (&str, &str, &str)) {
            let source = TrustedCoreSlotLayoutV1::new(&self.0, target())
                .source_root()
                .to_path_buf();
            std::fs::create_dir_all(&source).unwrap();
            std::fs::write(
                source.join("Cone.toml"),
                format!(
                    "schema = 1\n[cone]\ngroup = {:?}\nname = {:?}\nversion = {:?}\nkind = \"library\"\n",
                    coordinate.0, coordinate.1, coordinate.2
                ),
            )
            .unwrap();
        }

        fn artifact_path(&self) -> PathBuf {
            TrustedCoreSlotLayoutV1::new(&self.0, target())
                .artifact()
                .to_path_buf()
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn target() -> ValidatedLirTargetSelection {
        ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
    }

    #[test]
    fn resolver_mints_bootstrap_authority_for_the_exact_workspace_layout() {
        let sysroot = TempDirectory::new();
        sysroot.write_core_manifest(("scoop", "scoop.core", "0.1.0"));

        let slot = resolve_trusted_core_slot_at(&sysroot.0, target()).unwrap();
        assert_eq!(slot.artifact().path(), sysroot.artifact_path());
        let (bootstrap, artifact) = slot.into_bootstrap_parts();
        let source = bootstrap.source_slot();
        let authority = bootstrap.authority();
        assert_eq!(
            source.manifest().parsed().semantic().coordinate(),
            &ConeCoordinate::reserved_core()
        );
        assert_eq!(authority.expected_identity(), ConeIdentity::CORE);
        assert_eq!(authority.source_root(), source.manifest().real_root());
        assert_eq!(authority.artifact_path(), artifact.path());
        assert_eq!(authority.target(), target());
        assert_eq!(
            authority.toolchain_compatibility(),
            artifact.toolchain_compatibility()
        );
    }

    #[test]
    fn resolver_rejects_a_non_core_source_manifest() {
        let sysroot = TempDirectory::new();
        sysroot.write_core_manifest(("dev.example", "fake", "1.0.0"));

        assert!(matches!(
            resolve_trusted_core_slot_at(&sysroot.0, target())
                .unwrap_err()
                .kind(),
            TrustedCoreSlotErrorKind::SourceManifest(error)
                if matches!(
                    error.kind(),
                    scoop_manifest::ManifestRootErrorKind::Parse(parse)
                        if parse.kind()
                            == &scoop_manifest::ManifestParseErrorKind::TrustedCoreCoordinateMismatch
                )
        ));
    }

    #[test]
    fn artifact_input_requires_the_configured_regular_file() {
        let sysroot = TempDirectory::new();
        sysroot.write_core_manifest(("scoop", "scoop.core", "0.1.0"));
        let configured = sysroot.artifact_path();
        std::fs::create_dir_all(configured.parent().unwrap()).unwrap();
        std::fs::write(&configured, b"configured core").unwrap();
        let other = sysroot.0.join("other.slib");
        std::fs::write(&other, b"impostor").unwrap();

        let slot = resolve_trusted_core_slot_at(&sysroot.0, target()).unwrap();
        let input = slot.existing_artifact_input().unwrap();
        assert_eq!(input.path(), configured);
        assert_eq!(
            input.expected_coordinate(),
            &ConeCoordinate::reserved_core()
        );
        assert_eq!(input.target(), target());
        assert!(matches!(
            slot.existing_artifact_input_at(&other),
            Err(TrustedCoreArtifactInputError::WrongSlot { .. })
        ));
    }
}
