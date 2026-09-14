use std::fmt;
use std::path::PathBuf;

use scoop_ast::{CurrentConeParsedSources, NonEmptyVec};
use scoop_identity::ConeCoordinate;
use scoop_manifest::{
    DiscoveredManifestSources, DiscoveredSource, LoadedConeManifest, ManifestRootError,
    ManifestSpan, SingleFileInputError, SingleFileLocator, SourceDiscoveryError,
    discover_manifest_sources, load_cone_manifest, load_single_file_source,
};
use scoop_parser::{CurrentConeSourceInput, ParseCurrentConeError, parse_current_cone};
use scoop_wire::DecodeLimits;

use super::{
    CurrentConeInput, DiagnosticOutputPolicy, ExplicitDependencyInputs, SingleConeBuildRequest,
    SlibOutputDestination, StageDumpPolicy, TrustedCoreInput,
};
use crate::{
    LoadedTrustedCoreArtifact, TrustedCoreArtifactLoadError, TrustedCoreArtifactSlot,
    TrustedCoreArtifactValidationError, TrustedCoreBootstrapInput, ValidatedTrustedCoreArtifact,
};

/// The complete dependency input set supported by M23-3.
///
/// Its private constructor proves that neither request arguments nor the
/// current manifest contain a non-core dependency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedExplicitDependencyInputSet {
    _core_only: (),
}

impl ValidatedExplicitDependencyInputSet {
    pub const fn is_empty(self) -> bool {
        true
    }
}

#[derive(Debug)]
pub enum LoadedCurrentConeInput {
    Manifest { manifest: LoadedConeManifest },
    SingleFile { source: SingleFileLocator },
    TrustedCoreBootstrap { input: TrustedCoreBootstrapInput },
}

#[derive(Debug)]
pub enum LoadedTrustedCoreInput {
    Artifact(LoadedTrustedCoreArtifact),
    BootstrapSelf {
        artifact_slot: TrustedCoreArtifactSlot,
    },
}

#[derive(Debug)]
pub struct LoadedSingleConeBuildRequest {
    current: LoadedCurrentConeInput,
    dependencies: ValidatedExplicitDependencyInputSet,
    trusted_core: LoadedTrustedCoreInput,
    target: scoop_codegen::ResolvedTargetProfile,
    output: SlibOutputDestination,
    diagnostics: DiagnosticOutputPolicy,
    emit: StageDumpPolicy,
}

impl SingleConeBuildRequest {
    /// Loads only the current manifest and trusted artifact bytes. Current
    /// source discovery and parsing are deliberately unavailable before this
    /// method returns the core-only dependency proof.
    pub fn load_preflight(
        self,
        limits: DecodeLimits,
    ) -> Result<LoadedSingleConeBuildRequest, SingleConePreflightError> {
        let Self {
            current,
            dependencies,
            trusted_core,
            target,
            output,
            diagnostics,
            emit,
        } = self;
        let current = load_current_input(current)?;
        let dependencies = validate_core_only_dependencies(&current, &dependencies)?;
        let trusted_core = match trusted_core {
            TrustedCoreInput::Artifact(input) => {
                LoadedTrustedCoreInput::Artifact(input.load(limits).map_err(|source| {
                    SingleConePreflightError::TrustedCoreLoad(Box::new(source))
                })?)
            }
            TrustedCoreInput::BootstrapSelf { artifact_slot } => {
                LoadedTrustedCoreInput::BootstrapSelf { artifact_slot }
            }
        };
        Ok(LoadedSingleConeBuildRequest {
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

fn load_current_input(
    current: CurrentConeInput,
) -> Result<LoadedCurrentConeInput, SingleConePreflightError> {
    match current {
        CurrentConeInput::Manifest { root } => load_cone_manifest(&root)
            .map(|manifest| LoadedCurrentConeInput::Manifest { manifest })
            .map_err(|source| SingleConePreflightError::Manifest(Box::new(source))),
        CurrentConeInput::SingleFile { source } => {
            Ok(LoadedCurrentConeInput::SingleFile { source })
        }
        CurrentConeInput::TrustedCoreBootstrap { input } => {
            Ok(LoadedCurrentConeInput::TrustedCoreBootstrap { input: *input })
        }
    }
}

fn validate_core_only_dependencies(
    current: &LoadedCurrentConeInput,
    dependencies: &ExplicitDependencyInputs,
) -> Result<ValidatedExplicitDependencyInputSet, SingleConePreflightError> {
    if let LoadedCurrentConeInput::Manifest { manifest } = current
        && let Some((key, coordinate)) = manifest.parsed().semantic().dependencies().iter().next()
    {
        let declaration = manifest
            .parsed()
            .diagnostic_spans()
            .dependency(key)
            .ok_or_else(|| SingleConePreflightError::MissingManifestDependencySpan {
                coordinate: coordinate.clone(),
            })?
            .declaration()
            .clone();
        return Err(SingleConePreflightError::NonCoreDependencyUnavailable(
            NonCoreDependencyInput::Manifest {
                coordinate: coordinate.clone(),
                declaration,
            },
        ));
    }
    if let Some((index, artifact)) = dependencies.direct().iter().enumerate().next() {
        return Err(SingleConePreflightError::NonCoreDependencyUnavailable(
            NonCoreDependencyInput::DirectArtifact {
                index,
                path: artifact.as_path().to_path_buf(),
            },
        ));
    }
    if let Some((index, artifact)) = dependencies.support().iter().enumerate().next() {
        return Err(SingleConePreflightError::NonCoreDependencyUnavailable(
            NonCoreDependencyInput::SupportArtifact {
                index,
                path: artifact.as_path().to_path_buf(),
            },
        ));
    }
    Ok(ValidatedExplicitDependencyInputSet { _core_only: () })
}

impl LoadedSingleConeBuildRequest {
    /// Constructs the trusted-core proof before exposing any current source
    /// loading or parser entry.
    pub fn validate(
        &self,
    ) -> Result<ValidatedCoreOnlyBuildRequest<'_>, TrustedCoreArtifactValidationError> {
        let trusted_core = match &self.trusted_core {
            LoadedTrustedCoreInput::Artifact(artifact) => {
                ValidatedTrustedCoreInput::Artifact(Box::new(artifact.validate(&self.target)?))
            }
            LoadedTrustedCoreInput::BootstrapSelf { artifact_slot } => {
                ValidatedTrustedCoreInput::BootstrapSelf { artifact_slot }
            }
        };
        Ok(ValidatedCoreOnlyBuildRequest {
            request: self,
            trusted_core,
        })
    }
}

pub enum ValidatedTrustedCoreInput<'input> {
    Artifact(Box<ValidatedTrustedCoreArtifact<'input>>),
    BootstrapSelf {
        artifact_slot: &'input TrustedCoreArtifactSlot,
    },
}

pub struct ValidatedCoreOnlyBuildRequest<'input> {
    request: &'input LoadedSingleConeBuildRequest,
    trusted_core: ValidatedTrustedCoreInput<'input>,
}

impl<'input> ValidatedCoreOnlyBuildRequest<'input> {
    pub const fn current(&self) -> &LoadedCurrentConeInput {
        &self.request.current
    }

    pub const fn dependencies(&self) -> ValidatedExplicitDependencyInputSet {
        self.request.dependencies
    }

    pub const fn trusted_core(&self) -> &ValidatedTrustedCoreInput<'input> {
        &self.trusted_core
    }

    pub const fn target(&self) -> &scoop_codegen::ResolvedTargetProfile {
        &self.request.target
    }

    pub const fn output(&self) -> &SlibOutputDestination {
        &self.request.output
    }

    pub const fn diagnostics(&self) -> DiagnosticOutputPolicy {
        self.request.diagnostics
    }

    pub const fn emit(&self) -> StageDumpPolicy {
        self.request.emit
    }

    pub fn parse_current_sources<'request>(
        &'request self,
    ) -> Result<ParsedSingleConeBuildRequest<'request, 'input>, CurrentConeSourceStageError> {
        let sources = parse_loaded_current(&self.request.current)?;
        Ok(ParsedSingleConeBuildRequest {
            request: self,
            sources,
        })
    }
}

pub struct ParsedSingleConeBuildRequest<'request, 'artifact> {
    request: &'request ValidatedCoreOnlyBuildRequest<'artifact>,
    sources: CurrentConeParsedSources,
}

impl<'request, 'artifact> ParsedSingleConeBuildRequest<'request, 'artifact> {
    pub const fn request(&self) -> &'request ValidatedCoreOnlyBuildRequest<'artifact> {
        self.request
    }

    pub const fn sources(&self) -> &CurrentConeParsedSources {
        &self.sources
    }
}

fn parse_loaded_current(
    current: &LoadedCurrentConeInput,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    match current {
        LoadedCurrentConeInput::Manifest { manifest } => {
            let sources = discover_manifest_sources(manifest)
                .map_err(|source| CurrentConeSourceStageError::Discovery(Box::new(source)))?;
            parse_discovered_sources(&sources)
        }
        LoadedCurrentConeInput::SingleFile { source } => {
            let source = load_single_file_source(source)
                .map_err(|source| CurrentConeSourceStageError::SingleFile(Box::new(source)))?;
            parse_single_discovered_source(&source)
        }
        LoadedCurrentConeInput::TrustedCoreBootstrap { input } => {
            let sources = discover_manifest_sources(input.source_slot().manifest())
                .map_err(|source| CurrentConeSourceStageError::Discovery(Box::new(source)))?;
            parse_discovered_sources(&sources)
        }
    }
}

fn parse_discovered_sources(
    sources: &DiscoveredManifestSources,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    let first = parser_source_input(sources.first());
    let rest = sources.iter().skip(1).map(parser_source_input).collect();
    parse_current_cone(NonEmptyVec::new(first, rest))
        .map_err(|source| CurrentConeSourceStageError::Parser(Box::new(source)))
}

fn parse_single_discovered_source(
    source: &DiscoveredSource,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    parse_current_cone(NonEmptyVec::new(parser_source_input(source), Vec::new()))
        .map_err(|source| CurrentConeSourceStageError::Parser(Box::new(source)))
}

fn parser_source_input(source: &DiscoveredSource) -> CurrentConeSourceInput<'_> {
    CurrentConeSourceInput::new(
        source.identity(),
        source.source_text(),
        source.display_locator().as_path(),
    )
}

#[derive(Debug)]
pub enum CurrentConeSourceStageError {
    Discovery(Box<SourceDiscoveryError>),
    SingleFile(Box<SingleFileInputError>),
    Parser(Box<ParseCurrentConeError>),
}

impl fmt::Display for CurrentConeSourceStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discovery(source) => source.fmt(formatter),
            Self::SingleFile(source) => source.fmt(formatter),
            Self::Parser(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CurrentConeSourceStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Discovery(source) => source.as_ref(),
            Self::SingleFile(source) => source.as_ref(),
            Self::Parser(source) => source.as_ref(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NonCoreDependencyInput {
    Manifest {
        coordinate: ConeCoordinate,
        declaration: ManifestSpan,
    },
    DirectArtifact {
        index: usize,
        path: PathBuf,
    },
    SupportArtifact {
        index: usize,
        path: PathBuf,
    },
}

impl NonCoreDependencyInput {
    pub const CODE: &'static str = "SCOOPC_CAPABILITY_NON_CORE_DEPENDENCY_UNAVAILABLE";
}

impl fmt::Display for NonCoreDependencyInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest { coordinate, .. } => {
                write!(formatter, "manifest dependency {coordinate}")
            }
            Self::DirectArtifact { index, path } => {
                write!(formatter, "direct artifact {index} {}", path.display())
            }
            Self::SupportArtifact { index, path } => {
                write!(formatter, "support artifact {index} {}", path.display())
            }
        }
    }
}

#[derive(Debug)]
pub enum SingleConePreflightError {
    Manifest(Box<ManifestRootError>),
    MissingManifestDependencySpan { coordinate: ConeCoordinate },
    NonCoreDependencyUnavailable(NonCoreDependencyInput),
    TrustedCoreLoad(Box<TrustedCoreArtifactLoadError>),
}

impl fmt::Display for SingleConePreflightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(source) => source.fmt(formatter),
            Self::MissingManifestDependencySpan { coordinate } => write!(
                formatter,
                "manifest dependency {coordinate} has no diagnostic declaration span"
            ),
            Self::NonCoreDependencyUnavailable(input) => write!(
                formatter,
                "{}: {input} requires non-core dependency support unavailable in M23-3",
                NonCoreDependencyInput::CODE
            ),
            Self::TrustedCoreLoad(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for SingleConePreflightError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Manifest(source) => Some(source.as_ref()),
            Self::TrustedCoreLoad(source) => Some(source.as_ref()),
            Self::MissingManifestDependencySpan { .. } | Self::NonCoreDependencyUnavailable(_) => {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::SourceIdentity;
    use scoop_manifest::ManifestRootLocator;

    use super::*;
    use crate::HostArtifactLocator;

    #[test]
    fn manifest_dependency_is_rejected_with_coordinate_and_original_span() {
        let directory = tempfile::tempdir().unwrap();
        let manifest_path = directory.path().join("Cone.toml");
        let text = "schema = 1\n[cone]\ngroup = \"test\"\nname = \"current\"\nversion = \"0.0.0\"\nkind = \"library\"\n[dependencies]\n\"dev.example:dep\" = \"1.2.3\"\n";
        std::fs::write(&manifest_path, text).unwrap();
        let current = load_current_input(CurrentConeInput::Manifest {
            root: ManifestRootLocator::exact_manifest_file(manifest_path),
        })
        .unwrap();
        let dependencies = ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap();

        let error = validate_core_only_dependencies(&current, &dependencies).unwrap_err();

        assert!(matches!(
            error,
            SingleConePreflightError::NonCoreDependencyUnavailable(
                NonCoreDependencyInput::Manifest {
                    coordinate,
                    declaration,
                }
            ) if coordinate
                == ConeCoordinate::new("dev.example", "dep", "1.2.3").unwrap()
                && &text[declaration.range()] == "\"1.2.3\""
        ));
        assert!(!directory.path().join("src").exists());
    }

    #[test]
    fn explicit_artifact_is_rejected_without_reading_it() {
        let source_directory = tempfile::tempdir().unwrap();
        let source_path = source_directory.path().join("main.scoop");
        std::fs::write(&source_path, "fun main() {}\n").unwrap();
        let current = load_current_input(CurrentConeInput::SingleFile {
            source: SingleFileLocator::from_path(source_path).unwrap(),
        })
        .unwrap();
        let missing = source_directory.path().join("missing.slib");
        let dependencies = ExplicitDependencyInputs::new(
            vec![HostArtifactLocator::new(&missing).unwrap()],
            Vec::new(),
        )
        .unwrap();

        assert!(matches!(
            validate_core_only_dependencies(&current, &dependencies),
            Err(SingleConePreflightError::NonCoreDependencyUnavailable(
                NonCoreDependencyInput::DirectArtifact { index: 0, path }
            )) if path == missing
        ));
    }

    #[test]
    fn empty_dependency_input_constructs_the_only_success_state() {
        let source_directory = tempfile::tempdir().unwrap();
        let source_path = source_directory.path().join("main.scoop");
        std::fs::write(&source_path, "fun main() {}\n").unwrap();
        let current = load_current_input(CurrentConeInput::SingleFile {
            source: SingleFileLocator::from_path(source_path).unwrap(),
        })
        .unwrap();
        let dependencies = ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap();

        assert!(
            validate_core_only_dependencies(&current, &dependencies)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn manifest_sources_parse_in_canonical_identity_order() {
        let directory = manifest_cone();
        std::fs::write(directory.path().join("src/z.scoop"), "package z\n").unwrap();
        std::fs::write(directory.path().join("src/a.scoop"), "package a\n").unwrap();
        let current = load_current_input(CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(directory.path()),
        })
        .unwrap();

        let parsed = parse_loaded_current(&current).unwrap();

        let paths = parsed
            .sources()
            .sources()
            .iter()
            .map(|source| source.identity().logical_path().as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, ["src/a.scoop", "src/z.scoop"]);
        assert_eq!(parsed.source_texts().entries().len(), 2);
        assert_eq!(parsed.diagnostic_context().entries().len(), 2);
    }

    #[test]
    fn single_file_parse_uses_the_reserved_source_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("arbitrary-name.scoop");
        std::fs::write(&path, "fun main() {}\n").unwrap();
        let current = load_current_input(CurrentConeInput::SingleFile {
            source: SingleFileLocator::from_path(&path).unwrap(),
        })
        .unwrap();

        let parsed = parse_loaded_current(&current).unwrap();

        assert_eq!(parsed.cone(), scoop_identity::ConeIdentity::SINGLE_FILE);
        assert_eq!(
            parsed.sources().sources().first().identity(),
            &SourceIdentity::single_file()
        );
        assert_eq!(
            parsed
                .diagnostic_context()
                .entries()
                .first()
                .display_locator(),
            path
        );
    }

    #[test]
    fn parser_failure_keeps_semantic_source_identity() {
        let directory = manifest_cone();
        std::fs::write(directory.path().join("src/bad.scoop"), "package .bad\n").unwrap();
        let current = load_current_input(CurrentConeInput::Manifest {
            root: ManifestRootLocator::cone_directory(directory.path()),
        })
        .unwrap();

        let error = parse_loaded_current(&current).unwrap_err();

        assert!(matches!(
            error,
            CurrentConeSourceStageError::Parser(source)
                if matches!(source.as_ref(), ParseCurrentConeError::Diagnostics(diagnostics)
                    if diagnostics.len() == 1
                        && diagnostics[0].identity().logical_path().as_str() == "src/bad.scoop")
        ));
    }

    fn manifest_cone() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("src")).unwrap();
        std::fs::write(
            directory.path().join("Cone.toml"),
            "schema = 1\n[cone]\ngroup = \"test\"\nname = \"current\"\nversion = \"0.0.0\"\nkind = \"library\"\n",
        )
        .unwrap();
        directory
    }
}
