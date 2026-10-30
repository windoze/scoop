use std::fmt;
use std::path::{Path, PathBuf};

use scoop_manifest::SingleFileLocator;
use scoop_protocol::package::PreparedResolutionV1;
use scoop_protocol::{CompilationPurpose, TargetSelectionRequestV1};

use scoop_toolchain::{ResolvedTargetProfile, ToolchainError};

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
    Prepared {
        identity: scoop_identity::ConeIdentity,
        resolution: Box<PreparedResolutionV1>,
    },
    SingleFile(SingleFileLocator),
}

impl BuildRootInput {
    /// Accept an exact package graph already decoded at the PM protocol boundary.
    pub fn prepared(resolution: PreparedResolutionV1) -> Result<Self, BuildRootInputError> {
        let identity = resolution
            .coordinate
            .identity()
            .map_err(BuildRootInputError::Identity)?;
        Ok(Self(BuildRootInputKind::Prepared {
            identity,
            resolution: Box::new(resolution),
        }))
    }

    pub fn single_file(source: SingleFileLocator) -> Self {
        debug_assert!(source.resolved_path().is_absolute());
        Self(BuildRootInputKind::SingleFile(source))
    }

    pub const fn single_file_source(&self) -> Option<&SingleFileLocator> {
        match &self.0 {
            BuildRootInputKind::Prepared { .. } => None,
            BuildRootInputKind::SingleFile(source) => Some(source),
        }
    }

    pub(crate) fn into_kind(self) -> BuildRootInputKind {
        self.0
    }
}

#[derive(Debug)]
pub enum BuildRootInputError {
    Identity(scoop_wire::HashError),
}

impl fmt::Display for BuildRootInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BuildRootInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Identity(error) => Some(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticsPolicy {
    Human,
    Structured,
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
    optimization: scoop_lir::OptimizationMode,
    debug_info: scoop_lir::DebugInfoLevel,
    purpose: CompilationPurpose,
    jobs: std::num::NonZeroUsize,
    generators: scoop_toolchain::GeneratorPrograms,
}

pub(crate) struct BuildGraphRequestParts {
    pub(crate) root: BuildRootInput,
    pub(crate) artifact_search_roots: Vec<ArtifactSearchRoot>,
    pub(crate) cache_root: ArtifactCacheRoot,
    pub(crate) sysroot: TrustedSysrootRoot,
    pub(crate) target: ResolvedTargetProfile,
    pub(crate) compiler: PairedScoopcLocator,
    pub(crate) diagnostics: DiagnosticsPolicy,
    pub(crate) optimization: scoop_lir::OptimizationMode,
    pub(crate) debug_info: scoop_lir::DebugInfoLevel,
    pub(crate) purpose: CompilationPurpose,
    pub(crate) jobs: std::num::NonZeroUsize,
    pub(crate) generators: scoop_toolchain::GeneratorPrograms,
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
    ) -> Result<Self, BuildGraphRequestError> {
        artifact_search_roots.sort_by(|left, right| left.as_path().cmp(right.as_path()));
        artifact_search_roots.dedup();
        let target = ResolvedTargetProfile::resolve_request_cached(&target, cache_root.as_path())
            .map_err(BuildGraphRequestError::Toolchain)?;

        Ok(Self {
            root,
            artifact_search_roots,
            cache_root,
            sysroot,
            target,
            compiler,
            diagnostics,
            optimization: scoop_lir::OptimizationMode::Debug,
            debug_info: scoop_lir::DebugInfoLevel::None,
            purpose: CompilationPurpose::Production,
            jobs: std::thread::available_parallelism().unwrap_or(std::num::NonZeroUsize::MIN),
            generators: Default::default(),
        })
    }

    pub fn with_optimization(mut self, mode: scoop_lir::OptimizationMode) -> Self {
        self.optimization = mode;
        self
    }

    pub fn with_debug_info(mut self, level: scoop_lir::DebugInfoLevel) -> Self {
        self.debug_info = level;
        self
    }

    pub fn with_generators(mut self, generators: scoop_toolchain::GeneratorPrograms) -> Self {
        self.generators = generators;
        self
    }

    pub fn with_jobs(mut self, jobs: std::num::NonZeroUsize) -> Self {
        self.jobs = jobs;
        self
    }

    pub fn with_purpose(
        mut self,
        purpose: CompilationPurpose,
    ) -> Result<Self, BuildGraphRequestError> {
        if purpose == CompilationPurpose::Test && self.root.single_file_source().is_some() {
            return Err(BuildGraphRequestError::TestRequiresManifest);
        }
        self.purpose = purpose;
        Ok(self)
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

    pub(crate) fn into_parts(self) -> BuildGraphRequestParts {
        BuildGraphRequestParts {
            root: self.root,
            artifact_search_roots: self.artifact_search_roots,
            cache_root: self.cache_root,
            sysroot: self.sysroot,
            target: self.target,
            compiler: self.compiler,
            diagnostics: self.diagnostics,
            optimization: self.optimization,
            debug_info: self.debug_info,
            purpose: self.purpose,
            jobs: self.jobs,
            generators: self.generators,
        }
    }
}

#[derive(Debug)]
pub enum BuildGraphRequestError {
    TestRequiresManifest,
    Toolchain(ToolchainError),
}

impl fmt::Display for BuildGraphRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toolchain(error) => error.fmt(formatter),
            Self::TestRequiresManifest => formatter.write_str("test requires a manifest Cone"),
        }
    }
}

impl std::error::Error for BuildGraphRequestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Toolchain(error) => Some(error),
            Self::TestRequiresManifest => None,
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
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("main.scoop");
        std::fs::write(&source, "fun main() {}\n").unwrap();
        let make = |target: &str, roots: [&str; 3]| {
            BuildGraphRequest::new(
                BuildRootInput::single_file(SingleFileLocator::from_path(&source).unwrap()),
                roots
                    .map(|root| ArtifactSearchRoot::new(absolute(root)).unwrap())
                    .into(),
                ArtifactCacheRoot::new(absolute("cache")).unwrap(),
                TrustedSysrootRoot::new(absolute("sysroot")).unwrap(),
                TargetSelectionRequestV1::new(target.into()).unwrap(),
                PairedScoopcLocator::new(absolute("bin/scoopc")).unwrap(),
                DiagnosticsPolicy::Human,
            )
            .unwrap()
        };
        let canonical = scoop_toolchain::host_target_triple().unwrap();
        let alias = match crate::test_artifacts::host_target().target().id() {
            scoop_target_data::DARWIN_AARCH64_ID => "arm64-apple-macosx14.0",
            scoop_target_data::DARWIN_X86_64_ID => "x86_64-apple-macosx27.0.1",
            scoop_target_data::LINUX_X86_64_GNU_ID => "x86_64-linux-gnu",
            scoop_target_data::LINUX_X86_64_MUSL_ID => "x86_64-linux-musl",
            scoop_target_data::WINDOWS_X86_64_MSVC_UCRT_ID
            | scoop_target_data::WINDOWS_X86_64_MINGW_MSVCRT_ID
            | scoop_target_data::WINDOWS_X86_64_MINGW_UCRT_ID => canonical,
            _ => panic!("test host must be supported by the driver"),
        };
        let first = make(canonical, ["z", "a", "a"]);
        let second = make(alias, ["a", "z", "a"]);

        assert_eq!(first.target(), second.target());
        assert_eq!(
            first.artifact_search_roots(),
            second.artifact_search_roots()
        );
        assert_eq!(first.artifact_search_roots().len(), 2);
    }
}
