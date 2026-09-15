use std::fmt;
use std::path::{Path, PathBuf};

mod output;
mod preflight;
mod report;
use output::validate_output_isolation;
pub use output::{OutputAliasRole, OutputIsolationErrorKind, SlibOutputDestination};
pub use preflight::*;
pub use report::{
    CurrentConeDiagnosticSet, CurrentConeDiagnosticSetError, EmittedStageDump,
    SingleConeProductionSuccess,
};

use scoop_codegen::{CodegenError, ResolvedTargetProfile};
use scoop_manifest::{
    ManifestRootError, ManifestRootLocator, SingleFileInputError, SingleFileLocator,
};
use scoop_protocol::{
    CurrentConeRequestV1, HostPathError, ScoopcBuildRequestV1, StageDumpKindV1, StageDumpPolicyV1,
    TrustedCoreRequestV1,
};

use crate::{
    TrustedCoreArtifactInput, TrustedCoreArtifactInputError, TrustedCoreArtifactSlot,
    TrustedCoreBootstrapInput, TrustedCoreSlotError, resolve_trusted_core_slot,
};

const MAX_EXPLICIT_ARTIFACTS_PER_ROLE: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostArtifactLocator(PathBuf);

impl HostArtifactLocator {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, SingleConeBuildRequestError> {
        let path = path.into();
        if path.as_os_str().is_empty() {
            return Err(SingleConeBuildRequestError::EmptyArtifactLocator);
        }
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplicitDependencyInputs {
    direct: Vec<HostArtifactLocator>,
    support: Vec<HostArtifactLocator>,
}

impl ExplicitDependencyInputs {
    pub fn new(
        direct: Vec<HostArtifactLocator>,
        support: Vec<HostArtifactLocator>,
    ) -> Result<Self, SingleConeBuildRequestError> {
        if direct.len() > MAX_EXPLICIT_ARTIFACTS_PER_ROLE {
            return Err(SingleConeBuildRequestError::TooManyDependencyInputs {
                role: "direct",
                actual: direct.len(),
            });
        }
        if support.len() > MAX_EXPLICIT_ARTIFACTS_PER_ROLE {
            return Err(SingleConeBuildRequestError::TooManyDependencyInputs {
                role: "support",
                actual: support.len(),
            });
        }
        Ok(Self { direct, support })
    }

    pub fn direct(&self) -> &[HostArtifactLocator] {
        &self.direct
    }

    pub fn support(&self) -> &[HostArtifactLocator] {
        &self.support
    }

    pub fn is_empty(&self) -> bool {
        self.direct.is_empty() && self.support.is_empty()
    }
}

#[derive(Debug)]
pub enum CurrentConeInput {
    Manifest {
        root: ManifestRootLocator,
    },
    SingleFile {
        source: SingleFileLocator,
    },
    TrustedCoreBootstrap {
        input: Box<TrustedCoreBootstrapInput>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrustedCoreInput {
    Artifact(TrustedCoreArtifactInput),
    BootstrapSelf {
        artifact_slot: TrustedCoreArtifactSlot,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticOutputPolicy {
    Human,
    Structured,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageDumpKind {
    Ast,
    Hir,
    Mir,
    Lir,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageDumpPolicy {
    None,
    Stage(StageDumpKind),
}

#[derive(Debug)]
pub struct SingleConeBuildRequest {
    current: CurrentConeInput,
    dependencies: ExplicitDependencyInputs,
    trusted_core: TrustedCoreInput,
    target: ResolvedTargetProfile,
    output: SlibOutputDestination,
    diagnostics: DiagnosticOutputPolicy,
    emit: StageDumpPolicy,
}

impl SingleConeBuildRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        current: CurrentConeInput,
        dependencies: ExplicitDependencyInputs,
        trusted_core: TrustedCoreInput,
        target: ResolvedTargetProfile,
        output: SlibOutputDestination,
        diagnostics: DiagnosticOutputPolicy,
        emit: StageDumpPolicy,
    ) -> Result<Self, SingleConeBuildRequestError> {
        validate_request_shape(&current, &dependencies, &trusted_core, &output)?;
        Ok(Self {
            current,
            dependencies,
            trusted_core,
            target,
            output,
            diagnostics,
            emit,
        })
    }
}

#[derive(Debug)]
pub enum SingleConeBuildRequestError {
    EmptyArtifactLocator,
    TooManyDependencyInputs {
        role: &'static str,
        actual: usize,
    },
    EmptyOutputDestination,
    InvalidOutputExtension {
        path: PathBuf,
    },
    CurrentDirectory(std::io::Error),
    InvalidCurrentCoreCombination,
    SingleFileHasDependencies,
    BootstrapHasDependencies,
    BootstrapSlotMismatch,
    BootstrapOutputMismatch,
    OutputIsolation {
        path: PathBuf,
        kind: OutputIsolationErrorKind,
    },
}

impl fmt::Display for SingleConeBuildRequestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyArtifactLocator => formatter.write_str("artifact locator must not be empty"),
            Self::TooManyDependencyInputs { role, actual } => write!(
                formatter,
                "too many {role} dependency inputs: limit 4096, found {actual}"
            ),
            Self::EmptyOutputDestination => {
                formatter.write_str(".slib output destination must not be empty")
            }
            Self::InvalidOutputExtension { path } => write!(
                formatter,
                ".slib output destination must use the exact .slib extension: {}",
                path.display()
            ),
            Self::CurrentDirectory(error) => {
                write!(formatter, "cannot resolve current directory: {error}")
            }
            Self::InvalidCurrentCoreCombination => formatter
                .write_str("current Cone input and trusted core input are not a permitted pair"),
            Self::SingleFileHasDependencies => {
                formatter.write_str("single-file input cannot carry direct or support artifacts")
            }
            Self::BootstrapHasDependencies => formatter
                .write_str("trusted core bootstrap cannot carry direct or support artifacts"),
            Self::BootstrapSlotMismatch => formatter.write_str(
                "trusted core bootstrap source, authority, and artifact slot do not match",
            ),
            Self::BootstrapOutputMismatch => formatter.write_str(
                "trusted core bootstrap output must be the configured target artifact slot",
            ),
            Self::OutputIsolation { path, kind } => {
                write!(formatter, "invalid .slib output {}: {kind}", path.display())
            }
        }
    }
}

impl std::error::Error for SingleConeBuildRequestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentDirectory(error) => Some(error),
            Self::OutputIsolation { kind, .. } => kind.source(),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct CurrentConeOperandError {
    path: PathBuf,
    kind: CurrentConeOperandErrorKind,
}

impl CurrentConeOperandError {
    fn new(path: PathBuf, kind: CurrentConeOperandErrorKind) -> Self {
        Self { path, kind }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn kind(&self) -> &CurrentConeOperandErrorKind {
        &self.kind
    }
}

#[derive(Debug)]
pub enum CurrentConeOperandErrorKind {
    Inspect(std::io::Error),
    UnsupportedFile,
    UnsupportedFileType,
    SingleFile(SingleFileInputError),
}

impl fmt::Display for CurrentConeOperandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid current Cone operand {}: ",
            self.path.display()
        )?;
        match &self.kind {
            CurrentConeOperandErrorKind::Inspect(error) => {
                write!(formatter, "cannot inspect: {error}")
            }
            CurrentConeOperandErrorKind::UnsupportedFile => formatter
                .write_str("regular file must be named Cone.toml or have exact .scoop extension"),
            CurrentConeOperandErrorKind::UnsupportedFileType => {
                formatter.write_str("operand must resolve to a directory or regular file")
            }
            CurrentConeOperandErrorKind::SingleFile(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CurrentConeOperandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            CurrentConeOperandErrorKind::Inspect(error) => Some(error),
            CurrentConeOperandErrorKind::SingleFile(error) => Some(error),
            CurrentConeOperandErrorKind::UnsupportedFile
            | CurrentConeOperandErrorKind::UnsupportedFileType => None,
        }
    }
}

#[derive(Debug)]
pub enum BuildRequestNormalizationError {
    CurrentOperand(CurrentConeOperandError),
    ManifestRoot(ManifestRootError),
    SingleFile(SingleFileInputError),
    HostPath(HostPathError),
    Target(CodegenError),
    TrustedCoreSlot(TrustedCoreSlotError),
    TrustedCoreArtifact(TrustedCoreArtifactInputError),
    Request(SingleConeBuildRequestError),
}

impl fmt::Display for BuildRequestNormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentOperand(error) => error.fmt(formatter),
            Self::ManifestRoot(error) => error.fmt(formatter),
            Self::SingleFile(error) => error.fmt(formatter),
            Self::HostPath(error) => error.fmt(formatter),
            Self::Target(error) => error.fmt(formatter),
            Self::TrustedCoreSlot(error) => error.fmt(formatter),
            Self::TrustedCoreArtifact(error) => error.fmt(formatter),
            Self::Request(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BuildRequestNormalizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentOperand(error) => Some(error),
            Self::ManifestRoot(error) => Some(error),
            Self::SingleFile(error) => Some(error),
            Self::HostPath(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::TrustedCoreSlot(error) => Some(error),
            Self::TrustedCoreArtifact(error) => Some(error),
            Self::Request(error) => Some(error),
        }
    }
}

pub fn classify_current_cone_operand(
    path: impl Into<PathBuf>,
) -> Result<CurrentConeInput, CurrentConeOperandError> {
    let path = path.into();
    let metadata = std::fs::metadata(&path).map_err(|error| {
        CurrentConeOperandError::new(path.clone(), CurrentConeOperandErrorKind::Inspect(error))
    })?;
    if metadata.is_dir() {
        return Ok(CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(path),
        });
    }
    if !metadata.is_file() {
        return Err(CurrentConeOperandError::new(
            path,
            CurrentConeOperandErrorKind::UnsupportedFileType,
        ));
    }
    if path.file_name().is_some_and(|name| name == "Cone.toml") {
        return Ok(CurrentConeInput::Manifest {
            root: ManifestRootLocator::exact_manifest_file(path),
        });
    }
    if path
        .extension()
        .is_some_and(|extension| extension == "scoop")
    {
        return SingleFileLocator::from_path(&path)
            .map(|source| CurrentConeInput::SingleFile { source })
            .map_err(|error| {
                CurrentConeOperandError::new(path, CurrentConeOperandErrorKind::SingleFile(error))
            });
    }
    Err(CurrentConeOperandError::new(
        path,
        CurrentConeOperandErrorKind::UnsupportedFile,
    ))
}

pub fn normalize_direct_build_request(
    operand: impl Into<PathBuf>,
    direct: Vec<PathBuf>,
    support: Vec<PathBuf>,
    output: impl Into<PathBuf>,
    diagnostics: DiagnosticOutputPolicy,
    emit: StageDumpPolicy,
) -> Result<SingleConeBuildRequest, BuildRequestNormalizationError> {
    let current = classify_current_cone_operand(operand)
        .map_err(BuildRequestNormalizationError::CurrentOperand)?;
    let dependencies = explicit_dependencies(direct, support)?;
    validate_dependency_shape_before_toolchain(&current, &dependencies)
        .map_err(BuildRequestNormalizationError::Request)?;
    let output =
        SlibOutputDestination::new(output).map_err(BuildRequestNormalizationError::Request)?;
    let target =
        ResolvedTargetProfile::resolve_host().map_err(BuildRequestNormalizationError::Target)?;
    let core_slot = resolve_trusted_core_slot(target.lir_target_selection())
        .map_err(BuildRequestNormalizationError::TrustedCoreSlot)?;
    let trusted_core = TrustedCoreInput::Artifact(
        core_slot
            .existing_artifact_input()
            .map_err(BuildRequestNormalizationError::TrustedCoreArtifact)?,
    );
    SingleConeBuildRequest::new(
        current,
        dependencies,
        trusted_core,
        target,
        output,
        diagnostics,
        emit,
    )
    .map_err(BuildRequestNormalizationError::Request)
}

pub fn normalize_protocol_build_request(
    build: &ScoopcBuildRequestV1,
) -> Result<SingleConeBuildRequest, BuildRequestNormalizationError> {
    let current_path = match build.current() {
        CurrentConeRequestV1::ManifestRoot { root } => {
            let path = root
                .to_path_buf()
                .map_err(BuildRequestNormalizationError::HostPath)?;
            Some((true, path))
        }
        CurrentConeRequestV1::SingleFile { source } => {
            let path = source
                .to_path_buf()
                .map_err(BuildRequestNormalizationError::HostPath)?;
            Some((false, path))
        }
        CurrentConeRequestV1::TrustedCoreBootstrap => None,
    };
    let dependencies = explicit_dependencies(
        protocol_paths(build.direct_slibs())?,
        protocol_paths(build.support_slibs())?,
    )?;
    let output_path = build
        .out_slib()
        .to_path_buf()
        .map_err(BuildRequestNormalizationError::HostPath)?;
    let output =
        SlibOutputDestination::new(output_path).map_err(BuildRequestNormalizationError::Request)?;
    let target = ResolvedTargetProfile::resolve(build.target().canonical_triple())
        .map_err(BuildRequestNormalizationError::Target)?;
    let core_slot = resolve_trusted_core_slot(target.lir_target_selection())
        .map_err(BuildRequestNormalizationError::TrustedCoreSlot)?;

    let (current, trusted_core) = match (current_path, build.trusted_core()) {
        (Some((true, path)), TrustedCoreRequestV1::ArtifactSlot { artifact }) => {
            let root = ManifestRootLocator::from_path(path)
                .map_err(BuildRequestNormalizationError::ManifestRoot)?;
            let artifact = artifact
                .to_path_buf()
                .map_err(BuildRequestNormalizationError::HostPath)?;
            let input = core_slot
                .existing_artifact_input_at(&artifact)
                .map_err(BuildRequestNormalizationError::TrustedCoreArtifact)?;
            (
                CurrentConeInput::Manifest { root },
                TrustedCoreInput::Artifact(input),
            )
        }
        (Some((false, path)), TrustedCoreRequestV1::ArtifactSlot { artifact }) => {
            let source = SingleFileLocator::from_path(path)
                .map_err(BuildRequestNormalizationError::SingleFile)?;
            let artifact = artifact
                .to_path_buf()
                .map_err(BuildRequestNormalizationError::HostPath)?;
            let input = core_slot
                .existing_artifact_input_at(&artifact)
                .map_err(BuildRequestNormalizationError::TrustedCoreArtifact)?;
            (
                CurrentConeInput::SingleFile { source },
                TrustedCoreInput::Artifact(input),
            )
        }
        (None, TrustedCoreRequestV1::Bootstrap) => {
            let (input, artifact_slot) = core_slot.into_bootstrap_parts();
            (
                CurrentConeInput::TrustedCoreBootstrap {
                    input: Box::new(input),
                },
                TrustedCoreInput::BootstrapSelf { artifact_slot },
            )
        }
        _ => {
            return Err(BuildRequestNormalizationError::Request(
                SingleConeBuildRequestError::InvalidCurrentCoreCombination,
            ));
        }
    };

    SingleConeBuildRequest::new(
        current,
        dependencies,
        trusted_core,
        target,
        output,
        map_diagnostic_policy(build.diagnostics()),
        map_dump_policy(build.emit()),
    )
    .map_err(BuildRequestNormalizationError::Request)
}

fn validate_request_shape(
    current: &CurrentConeInput,
    dependencies: &ExplicitDependencyInputs,
    trusted_core: &TrustedCoreInput,
    output: &SlibOutputDestination,
) -> Result<(), SingleConeBuildRequestError> {
    match (current, trusted_core) {
        (CurrentConeInput::Manifest { .. }, TrustedCoreInput::Artifact(_)) => Ok(()),
        (CurrentConeInput::SingleFile { .. }, TrustedCoreInput::Artifact(_)) => {
            if dependencies.is_empty() {
                Ok(())
            } else {
                Err(SingleConeBuildRequestError::SingleFileHasDependencies)
            }
        }
        (
            CurrentConeInput::TrustedCoreBootstrap { input },
            TrustedCoreInput::BootstrapSelf { artifact_slot },
        ) => {
            if !dependencies.is_empty() {
                return Err(SingleConeBuildRequestError::BootstrapHasDependencies);
            }
            if input.source_slot().manifest().real_root() != input.authority().source_root()
                || artifact_slot.path() != input.authority().artifact_path()
                || artifact_slot.target() != input.authority().target()
                || artifact_slot.toolchain_compatibility()
                    != input.authority().toolchain_compatibility()
            {
                return Err(SingleConeBuildRequestError::BootstrapSlotMismatch);
            }
            if output.as_path() != artifact_slot.path() {
                return Err(SingleConeBuildRequestError::BootstrapOutputMismatch);
            }
            Ok(())
        }
        _ => Err(SingleConeBuildRequestError::InvalidCurrentCoreCombination),
    }?;
    validate_output_isolation(current, dependencies, trusted_core, output)
}

fn validate_dependency_shape_before_toolchain(
    current: &CurrentConeInput,
    dependencies: &ExplicitDependencyInputs,
) -> Result<(), SingleConeBuildRequestError> {
    match current {
        CurrentConeInput::Manifest { .. } => Ok(()),
        CurrentConeInput::SingleFile { .. } if dependencies.is_empty() => Ok(()),
        CurrentConeInput::SingleFile { .. } => {
            Err(SingleConeBuildRequestError::SingleFileHasDependencies)
        }
        CurrentConeInput::TrustedCoreBootstrap { .. } if dependencies.is_empty() => Ok(()),
        CurrentConeInput::TrustedCoreBootstrap { .. } => {
            Err(SingleConeBuildRequestError::BootstrapHasDependencies)
        }
    }
}

fn explicit_dependencies(
    direct: Vec<PathBuf>,
    support: Vec<PathBuf>,
) -> Result<ExplicitDependencyInputs, BuildRequestNormalizationError> {
    let direct = direct
        .into_iter()
        .map(HostArtifactLocator::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(BuildRequestNormalizationError::Request)?;
    let support = support
        .into_iter()
        .map(HostArtifactLocator::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(BuildRequestNormalizationError::Request)?;
    ExplicitDependencyInputs::new(direct, support).map_err(BuildRequestNormalizationError::Request)
}

fn protocol_paths(
    paths: &[scoop_protocol::HostPathCarrier],
) -> Result<Vec<PathBuf>, BuildRequestNormalizationError> {
    paths
        .iter()
        .map(|path| {
            path.to_path_buf()
                .map_err(BuildRequestNormalizationError::HostPath)
        })
        .collect()
}

fn map_diagnostic_policy(
    policy: scoop_protocol::DiagnosticOutputPolicyV1,
) -> DiagnosticOutputPolicy {
    match policy {
        scoop_protocol::DiagnosticOutputPolicyV1::Human => DiagnosticOutputPolicy::Human,
        scoop_protocol::DiagnosticOutputPolicyV1::Structured => DiagnosticOutputPolicy::Structured,
    }
}

fn map_dump_policy(policy: StageDumpPolicyV1) -> StageDumpPolicy {
    match policy {
        StageDumpPolicyV1::None => StageDumpPolicy::None,
        StageDumpPolicyV1::Stage(stage) => StageDumpPolicy::Stage(match stage {
            StageDumpKindV1::Ast => StageDumpKind::Ast,
            StageDumpKindV1::Hir => StageDumpKind::Hir,
            StageDumpKindV1::Mir => StageDumpKind::Mir,
            StageDumpKindV1::Lir => StageDumpKind::Lir,
        }),
    }
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
                "scoop-single-cone-request-{}-{serial}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(std::fs::canonicalize(path).unwrap())
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn operand_classifier_uses_only_the_closed_file_shapes() {
        let directory = TempDirectory::new();
        let source = directory.0.join("main.scoop");
        let manifest = directory.0.join("Cone.toml");
        let other = directory.0.join("other.toml");
        std::fs::write(&source, "fun main() {}\n").unwrap();
        std::fs::write(&manifest, "manifest").unwrap();
        std::fs::write(&other, "other").unwrap();

        assert!(matches!(
            classify_current_cone_operand(&directory.0).unwrap(),
            CurrentConeInput::Manifest {
                root: ManifestRootLocator::ConeDirectory(_)
            }
        ));
        assert!(matches!(
            classify_current_cone_operand(&manifest).unwrap(),
            CurrentConeInput::Manifest {
                root: ManifestRootLocator::ExactConeManifestFile(_)
            }
        ));
        assert!(matches!(
            classify_current_cone_operand(&source).unwrap(),
            CurrentConeInput::SingleFile { .. }
        ));
        assert!(matches!(
            classify_current_cone_operand(&other).unwrap_err().kind(),
            CurrentConeOperandErrorKind::UnsupportedFile
        ));
    }

    #[test]
    fn explicit_dependency_roles_are_bounded_and_preserved() {
        let locator = HostArtifactLocator::new("dependency.slib").unwrap();
        let inputs = ExplicitDependencyInputs::new(
            vec![locator.clone()],
            vec![HostArtifactLocator::new("support.slib").unwrap()],
        )
        .unwrap();
        assert_eq!(inputs.direct()[0].as_path(), Path::new("dependency.slib"));
        assert_eq!(inputs.support()[0].as_path(), Path::new("support.slib"));

        assert!(matches!(
            ExplicitDependencyInputs::new(
                vec![locator; MAX_EXPLICIT_ARTIFACTS_PER_ROLE + 1],
                Vec::new(),
            ),
            Err(SingleConeBuildRequestError::TooManyDependencyInputs {
                role: "direct",
                actual
            }) if actual == MAX_EXPLICIT_ARTIFACTS_PER_ROLE + 1
        ));
    }

    #[test]
    fn single_file_dependencies_fail_before_toolchain_resolution() {
        let directory = TempDirectory::new();
        let source = directory.0.join("main.scoop");
        std::fs::write(&source, "fun main() {}\n").unwrap();

        assert!(matches!(
            normalize_direct_build_request(
                &source,
                vec![PathBuf::from("dependency.slib")],
                Vec::new(),
                directory.0.join("main.slib"),
                DiagnosticOutputPolicy::Human,
                StageDumpPolicy::None,
            ),
            Err(BuildRequestNormalizationError::Request(
                SingleConeBuildRequestError::SingleFileHasDependencies
            ))
        ));
    }

    #[test]
    fn output_destination_requires_the_exact_slib_extension() {
        assert!(matches!(
            SlibOutputDestination::new("output.bin"),
            Err(SingleConeBuildRequestError::InvalidOutputExtension { .. })
        ));
    }

    #[test]
    fn output_isolation_rejects_dependency_aliases_before_writing() {
        let directory = TempDirectory::new();
        let manifest = directory.0.join("Cone.toml");
        let core = directory.0.join("core.slib");
        let dependency = directory.0.join("dependency.slib");
        std::fs::write(&manifest, "manifest").unwrap();
        std::fs::write(&core, "core").unwrap();
        std::fs::write(&dependency, "dependency").unwrap();
        let current = CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(directory.0.clone()),
        };
        let dependencies = ExplicitDependencyInputs::new(
            vec![HostArtifactLocator::new(&dependency).unwrap()],
            Vec::new(),
        )
        .unwrap();
        let trusted_core = TrustedCoreInput::Artifact(TrustedCoreArtifactInput::for_test(core));
        let output = SlibOutputDestination::new(&dependency).unwrap();

        assert!(matches!(
            validate_output_isolation(&current, &dependencies, &trusted_core, &output),
            Err(SingleConeBuildRequestError::OutputIsolation {
                kind: OutputIsolationErrorKind::AliasesInput {
                    role: OutputAliasRole::DirectArtifact { index: 0 },
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn output_isolation_accepts_a_new_file_in_an_existing_directory() {
        let directory = TempDirectory::new();
        let manifest = directory.0.join("Cone.toml");
        let core = directory.0.join("core.slib");
        std::fs::write(&manifest, "manifest").unwrap();
        std::fs::write(&core, "core").unwrap();
        let current = CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(directory.0.clone()),
        };
        let dependencies = ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap();
        let trusted_core = TrustedCoreInput::Artifact(TrustedCoreArtifactInput::for_test(core));
        let output = SlibOutputDestination::new(directory.0.join("output.slib")).unwrap();

        validate_output_isolation(&current, &dependencies, &trusted_core, &output).unwrap();
    }

    #[test]
    fn policy_projection_is_total() {
        assert_eq!(
            map_diagnostic_policy(scoop_protocol::DiagnosticOutputPolicyV1::Structured),
            DiagnosticOutputPolicy::Structured
        );
        for (wire, expected) in [
            (StageDumpKindV1::Ast, StageDumpKind::Ast),
            (StageDumpKindV1::Hir, StageDumpKind::Hir),
            (StageDumpKindV1::Mir, StageDumpKind::Mir),
            (StageDumpKindV1::Lir, StageDumpKind::Lir),
        ] {
            assert_eq!(
                map_dump_policy(StageDumpPolicyV1::Stage(wire)),
                StageDumpPolicy::Stage(expected)
            );
        }
    }
}
