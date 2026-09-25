use std::fmt;
use std::path::{Path, PathBuf};

use scoop_manifest::{ManifestRootLocator, SingleFileLocator};
use scoop_protocol::TargetSelectionRequestV1;

use scoop_toolchain::{ResolvedTargetProfile, ToolchainError};
use scoop_wire::DecodeLimits;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSearchRoot(PathBuf);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactCacheRoot(PathBuf);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedSysrootRoot(PathBuf);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairedScoopcLocator(PathBuf);

macro_rules! absolute_locator {
    ($name:ident, $role:literal) => {
        impl $name {
            pub fn new(path: impl Into<PathBuf>) -> Result<Self, HostLocatorError> {
                let path = path.into();
                validate_absolute_path(&path, $role)?;
                Ok(Self(path))
            }

            pub fn as_path(&self) -> &Path {
                &self.0
            }
        }
    };
}

absolute_locator!(ArtifactSearchRoot, "artifact search root");
absolute_locator!(ArtifactCacheRoot, "artifact cache root");
absolute_locator!(TrustedSysrootRoot, "trusted sysroot");
absolute_locator!(PairedScoopcLocator, "paired scoopc executable");

fn validate_absolute_path(path: &Path, role: &'static str) -> Result<(), HostLocatorError> {
    if path.as_os_str().is_empty() {
        return Err(HostLocatorError {
            role,
            path: path.to_path_buf(),
            kind: HostLocatorErrorKind::Empty,
        });
    }
    if !path.is_absolute() {
        return Err(HostLocatorError {
            role,
            path: path.to_path_buf(),
            kind: HostLocatorErrorKind::NotAbsolute,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostLocatorErrorKind {
    Empty,
    NotAbsolute,
}

#[derive(Debug, Eq, PartialEq)]
pub struct HostLocatorError {
    role: &'static str,
    path: PathBuf,
    kind: HostLocatorErrorKind,
}

impl HostLocatorError {
    pub const fn role(&self) -> &'static str {
        self.role
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn kind(&self) -> HostLocatorErrorKind {
        self.kind
    }
}

impl fmt::Display for HostLocatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {} {}: ", self.role, self.path.display())?;
        formatter.write_str(match self.kind {
            HostLocatorErrorKind::Empty => "path is empty",
            HostLocatorErrorKind::NotAbsolute => "path must be absolute",
        })
    }
}

impl std::error::Error for HostLocatorError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildRootInput(BuildRootInputKind);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum BuildRootInputKind {
    ManifestCone(ManifestRootLocator),
    SingleFile(SingleFileLocator),
}

impl BuildRootInput {
    pub fn manifest(root: ManifestRootLocator) -> Result<Self, BuildRootInputError> {
        validate_absolute_path(root.as_path(), "build root")
            .map_err(BuildRootInputError::Locator)?;
        if matches!(&root, ManifestRootLocator::ExactConeManifestFile(path)
            if path.file_name().is_none_or(|name| name != "Cone.toml"))
        {
            return Err(BuildRootInputError::InvalidManifestFileName(
                root.as_path().to_path_buf(),
            ));
        }
        Ok(Self(BuildRootInputKind::ManifestCone(root)))
    }

    pub fn single_file(source: SingleFileLocator) -> Self {
        debug_assert!(source.resolved_path().is_absolute());
        Self(BuildRootInputKind::SingleFile(source))
    }

    pub const fn manifest_root(&self) -> Option<&ManifestRootLocator> {
        match &self.0 {
            BuildRootInputKind::ManifestCone(root) => Some(root),
            BuildRootInputKind::SingleFile(_) => None,
        }
    }

    pub const fn single_file_source(&self) -> Option<&SingleFileLocator> {
        match &self.0 {
            BuildRootInputKind::ManifestCone(_) => None,
            BuildRootInputKind::SingleFile(source) => Some(source),
        }
    }

    pub(crate) fn into_kind(self) -> BuildRootInputKind {
        self.0
    }
}

#[derive(Debug)]
pub enum BuildRootInputError {
    Locator(HostLocatorError),
    InvalidManifestFileName(PathBuf),
}

impl fmt::Display for BuildRootInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locator(error) => error.fmt(formatter),
            Self::InvalidManifestFileName(path) => write!(
                formatter,
                "invalid build root {}: manifest file must be named exactly Cone.toml",
                path.display()
            ),
        }
    }
}

impl std::error::Error for BuildRootInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Locator(error) => Some(error),
            Self::InvalidManifestFileName(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticsPolicy {
    Human,
    Structured,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BuildLimitsProfileV1 {
    artifact_decode: DecodeLimits,
}

impl BuildLimitsProfileV1 {
    pub const M23_DEFAULT: Self = Self {
        artifact_decode: DecodeLimits::M23_DEFAULT,
    };

    pub const fn artifact_decode(self) -> DecodeLimits {
        self.artifact_decode
    }
}

#[derive(Debug)]
pub struct BuildGraphRequest {
    root: BuildRootInput,
    artifact_search_roots: Vec<ArtifactSearchRoot>,
    cache_root: ArtifactCacheRoot,
    sysroot: TrustedSysrootRoot,
    target: ResolvedTargetProfile,
    compiler: PairedScoopcLocator,
    diagnostics: DiagnosticsPolicy,
    limits: BuildLimitsProfileV1,
}

pub(crate) struct BuildGraphRequestParts {
    pub(crate) root: BuildRootInput,
    pub(crate) artifact_search_roots: Vec<ArtifactSearchRoot>,
    pub(crate) cache_root: ArtifactCacheRoot,
    pub(crate) sysroot: TrustedSysrootRoot,
    pub(crate) target: ResolvedTargetProfile,
    pub(crate) compiler: PairedScoopcLocator,
    pub(crate) diagnostics: DiagnosticsPolicy,
    pub(crate) limits: BuildLimitsProfileV1,
}

impl BuildGraphRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        root: BuildRootInput,
        mut artifact_search_roots: Vec<ArtifactSearchRoot>,
        cache_root: ArtifactCacheRoot,
        sysroot: TrustedSysrootRoot,
        target: TargetSelectionRequestV1,
        compiler: PairedScoopcLocator,
        diagnostics: DiagnosticsPolicy,
        limits: BuildLimitsProfileV1,
    ) -> Result<Self, BuildGraphRequestError> {
        artifact_search_roots.sort_by(|left, right| left.as_path().cmp(right.as_path()));
        artifact_search_roots.dedup();
        let target = ResolvedTargetProfile::resolve(target.canonical_triple())
            .map_err(BuildGraphRequestError::Toolchain)?;

        Ok(Self {
            root,
            artifact_search_roots,
            cache_root,
            sysroot,
            target,
            compiler,
            diagnostics,
            limits,
        })
    }

    pub const fn root(&self) -> &BuildRootInput {
        &self.root
    }

    pub fn artifact_search_roots(&self) -> &[ArtifactSearchRoot] {
        &self.artifact_search_roots
    }

    pub const fn cache_root(&self) -> &ArtifactCacheRoot {
        &self.cache_root
    }

    pub const fn sysroot(&self) -> &TrustedSysrootRoot {
        &self.sysroot
    }

    pub const fn target(&self) -> &ResolvedTargetProfile {
        &self.target
    }

    pub const fn compiler(&self) -> &PairedScoopcLocator {
        &self.compiler
    }

    pub const fn diagnostics(&self) -> DiagnosticsPolicy {
        self.diagnostics
    }

    pub const fn limits(&self) -> BuildLimitsProfileV1 {
        self.limits
    }

    pub(crate) fn into_parts(self) -> BuildGraphRequestParts {
        BuildGraphRequestParts {
            root: self.root,
            artifact_search_roots: self.artifact_search_roots,
            cache_root: self.cache_root,
            sysroot: self.sysroot,
            target: self.target,
            compiler: self.compiler,
            diagnostics: self.diagnostics,
            limits: self.limits,
        }
    }
}

#[derive(Debug)]
pub enum BuildGraphRequestError {
    Toolchain(ToolchainError),
}

impl fmt::Display for BuildGraphRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toolchain(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BuildGraphRequestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Toolchain(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absolute(name: &str) -> PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn every_external_locator_requires_an_explicit_absolute_path() {
        for error in [
            ArtifactSearchRoot::new("search").unwrap_err(),
            ArtifactCacheRoot::new("cache").unwrap_err(),
            TrustedSysrootRoot::new("sysroot").unwrap_err(),
            PairedScoopcLocator::new("scoopc").unwrap_err(),
        ] {
            assert_eq!(error.kind(), HostLocatorErrorKind::NotAbsolute);
        }
    }

    #[test]
    fn manifest_root_is_lexically_closed_before_io() {
        let wrong = ManifestRootLocator::exact_manifest_file(absolute("cone.toml"));
        assert!(matches!(
            BuildRootInput::manifest(wrong),
            Err(BuildRootInputError::InvalidManifestFileName(_))
        ));

        let exact = ManifestRootLocator::exact_manifest_file(absolute("Cone.toml"));
        assert!(BuildRootInput::manifest(exact).is_ok());
        let directory = ManifestRootLocator::cone_directory(absolute("cone-root"));
        assert!(BuildRootInput::manifest(directory).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn host_locator_does_not_require_utf8() {
        use std::os::unix::ffi::OsStringExt;

        let mut path = absolute("non-utf8");
        path.push(std::ffi::OsString::from_vec(vec![0x66, 0x80]));
        assert_eq!(ArtifactSearchRoot::new(&path).unwrap().as_path(), path);
    }

    #[test]
    fn target_aliases_and_search_root_order_normalize_at_the_boundary() {
        let make = |target: &str, roots: [&str; 3]| {
            BuildGraphRequest::new(
                BuildRootInput::manifest(ManifestRootLocator::cone_directory(absolute("root")))
                    .unwrap(),
                roots
                    .map(|root| ArtifactSearchRoot::new(absolute(root)).unwrap())
                    .into(),
                ArtifactCacheRoot::new(absolute("cache")).unwrap(),
                TrustedSysrootRoot::new(absolute("sysroot")).unwrap(),
                TargetSelectionRequestV1::new(target.into()).unwrap(),
                PairedScoopcLocator::new(absolute("bin/scoopc")).unwrap(),
                DiagnosticsPolicy::Human,
                BuildLimitsProfileV1::M23_DEFAULT,
            )
            .unwrap()
        };
        let first = make("aarch64-apple-darwin", ["z", "a", "a"]);
        let second = make("arm64-apple-macosx14.0", ["a", "z", "a"]);

        assert_eq!(first.target(), second.target());
        assert_eq!(
            first.artifact_search_roots(),
            second.artifact_search_roots()
        );
        assert_eq!(first.artifact_search_roots().len(), 2);
    }
}
