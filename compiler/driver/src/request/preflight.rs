use std::fmt;
use std::path::PathBuf;

use scoop_ast::{CurrentConeParsedSources, NonEmptyVec};
use scoop_hir::CorePreludeImportError;
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
    CoreBootstrapAuthority, LoadedTrustedCoreArtifact, TrustedCoreArtifactLoadError,
    TrustedCoreArtifactSlot, TrustedCoreArtifactValidationError, TrustedCoreBootstrapInput,
    ValidatedTrustedCoreArtifact,
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
    ) -> Result<ValidatedCoreOnlyBuildRequest<'_>, CoreOnlyRequestValidationError> {
        let current = match (&self.current, &self.trusted_core) {
            (
                LoadedCurrentConeInput::Manifest { manifest },
                LoadedTrustedCoreInput::Artifact(artifact),
            ) => ValidatedCurrentConeInput::Manifest {
                manifest,
                trusted_core: Box::new(artifact.validate(&self.target).map_err(|source| {
                    CoreOnlyRequestValidationError::TrustedCore(Box::new(source))
                })?),
            },
            (
                LoadedCurrentConeInput::SingleFile { source },
                LoadedTrustedCoreInput::Artifact(artifact),
            ) => ValidatedCurrentConeInput::SingleFile {
                source,
                trusted_core: Box::new(artifact.validate(&self.target).map_err(|source| {
                    CoreOnlyRequestValidationError::TrustedCore(Box::new(source))
                })?),
            },
            (
                LoadedCurrentConeInput::TrustedCoreBootstrap { input },
                LoadedTrustedCoreInput::BootstrapSelf { artifact_slot },
            ) => ValidatedCurrentConeInput::TrustedCoreBootstrap {
                input,
                artifact_slot,
            },
            _ => return Err(CoreOnlyRequestValidationError::InvalidLoadedInputPair),
        };
        Ok(ValidatedCoreOnlyBuildRequest {
            request: self,
            current,
        })
    }
}

#[derive(Debug)]
pub enum CoreOnlyRequestValidationError {
    InvalidLoadedInputPair,
    TrustedCore(Box<TrustedCoreArtifactValidationError>),
}

impl fmt::Display for CoreOnlyRequestValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLoadedInputPair => formatter
                .write_str("loaded current Cone and trusted core inputs are not a permitted pair"),
            Self::TrustedCore(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreOnlyRequestValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidLoadedInputPair => None,
            Self::TrustedCore(source) => Some(source.as_ref()),
        }
    }
}

pub enum ValidatedCurrentConeInput<'input> {
    Manifest {
        manifest: &'input LoadedConeManifest,
        trusted_core: Box<ValidatedTrustedCoreArtifact<'input>>,
    },
    SingleFile {
        source: &'input SingleFileLocator,
        trusted_core: Box<ValidatedTrustedCoreArtifact<'input>>,
    },
    TrustedCoreBootstrap {
        input: &'input TrustedCoreBootstrapInput,
        artifact_slot: &'input TrustedCoreArtifactSlot,
    },
}

pub struct ValidatedCoreOnlyBuildRequest<'input> {
    request: &'input LoadedSingleConeBuildRequest,
    current: ValidatedCurrentConeInput<'input>,
}

impl<'input> ValidatedCoreOnlyBuildRequest<'input> {
    pub const fn current(&self) -> &ValidatedCurrentConeInput<'input> {
        &self.current
    }

    pub const fn dependencies(&self) -> ValidatedExplicitDependencyInputSet {
        self.request.dependencies
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
        match &self.current {
            ValidatedCurrentConeInput::Manifest {
                manifest,
                trusted_core,
            } => Ok(ParsedSingleConeBuildRequest::Ordinary(
                ParsedOrdinaryConeBuildRequest {
                    request: self,
                    trusted_core,
                    sources: parse_manifest_current(manifest)?,
                },
            )),
            ValidatedCurrentConeInput::SingleFile {
                source,
                trusted_core,
            } => Ok(ParsedSingleConeBuildRequest::Ordinary(
                ParsedOrdinaryConeBuildRequest {
                    request: self,
                    trusted_core,
                    sources: parse_single_file_current(source)?,
                },
            )),
            ValidatedCurrentConeInput::TrustedCoreBootstrap {
                input,
                artifact_slot,
            } => Ok(ParsedSingleConeBuildRequest::TrustedCoreBootstrap(
                ParsedCoreBootstrapBuildRequest {
                    request: self,
                    authority: input.authority(),
                    artifact_slot,
                    sources: parse_manifest_current(input.source_slot().manifest())?,
                },
            )),
        }
    }
}

pub enum ParsedSingleConeBuildRequest<'request, 'artifact> {
    Ordinary(ParsedOrdinaryConeBuildRequest<'request, 'artifact>),
    TrustedCoreBootstrap(ParsedCoreBootstrapBuildRequest<'request, 'artifact>),
}

impl<'request, 'artifact> ParsedSingleConeBuildRequest<'request, 'artifact> {
    pub const fn request(&self) -> &'request ValidatedCoreOnlyBuildRequest<'artifact> {
        match self {
            Self::Ordinary(parsed) => parsed.request,
            Self::TrustedCoreBootstrap(parsed) => parsed.request,
        }
    }

    pub const fn sources(&self) -> &CurrentConeParsedSources {
        match self {
            Self::Ordinary(parsed) => &parsed.sources,
            Self::TrustedCoreBootstrap(parsed) => &parsed.sources,
        }
    }
}

pub struct ParsedOrdinaryConeBuildRequest<'request, 'artifact> {
    request: &'request ValidatedCoreOnlyBuildRequest<'artifact>,
    trusted_core: &'request ValidatedTrustedCoreArtifact<'artifact>,
    sources: CurrentConeParsedSources,
}

impl<'request, 'artifact> ParsedOrdinaryConeBuildRequest<'request, 'artifact> {
    pub const fn request(&self) -> &'request ValidatedCoreOnlyBuildRequest<'artifact> {
        self.request
    }

    pub const fn sources(&self) -> &CurrentConeParsedSources {
        &self.sources
    }

    /// Binds the parsed current sources to the only HIR import capability
    /// granted by this request's exact trusted-core artifact.
    pub fn hir_input(
        &self,
    ) -> Result<scoop_hir_lower::OrdinaryCoreOnlySources<'_>, OrdinaryCoreOnlyHirInputError> {
        let core_prelude = self
            .trusted_core
            .import_core_prelude()
            .map_err(OrdinaryCoreOnlyHirInputError::CorePrelude)?;
        scoop_hir_lower::OrdinaryCoreOnlySources::try_new(&self.sources, core_prelude)
            .map_err(OrdinaryCoreOnlyHirInputError::Sources)
    }
}

#[derive(Debug)]
pub enum OrdinaryCoreOnlyHirInputError {
    CorePrelude(CorePreludeImportError),
    Sources(scoop_hir_lower::OrdinaryCoreOnlySourceError),
}

impl fmt::Display for OrdinaryCoreOnlyHirInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CorePrelude(source) => source.fmt(formatter),
            Self::Sources(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for OrdinaryCoreOnlyHirInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CorePrelude(source) => Some(source),
            Self::Sources(source) => Some(source),
        }
    }
}

pub struct ParsedCoreBootstrapBuildRequest<'request, 'artifact> {
    request: &'request ValidatedCoreOnlyBuildRequest<'artifact>,
    authority: &'request CoreBootstrapAuthority,
    artifact_slot: &'request TrustedCoreArtifactSlot,
    sources: CurrentConeParsedSources,
}

impl<'request, 'artifact> ParsedCoreBootstrapBuildRequest<'request, 'artifact> {
    pub const fn request(&self) -> &'request ValidatedCoreOnlyBuildRequest<'artifact> {
        self.request
    }

    pub const fn sources(&self) -> &CurrentConeParsedSources {
        &self.sources
    }

    pub fn hir_input(
        &self,
    ) -> Result<TrustedCoreBootstrapHirInput<'_>, CoreBootstrapHirInputError> {
        TrustedCoreBootstrapHirInput::try_new(&self.sources, self.authority, self.artifact_slot)
    }
}

/// Authority-bearing driver projection for the trusted-core HIR stage.
///
/// The source-only lowerer input cannot mint this value: it additionally
/// binds the resolver authority to the configured artifact slot that must
/// receive the eventual bootstrap artifact.
pub struct TrustedCoreBootstrapHirInput<'a> {
    sources: scoop_hir_lower::CoreBootstrapSources<'a>,
    authority: &'a CoreBootstrapAuthority,
    artifact_slot: &'a TrustedCoreArtifactSlot,
}

impl<'a> TrustedCoreBootstrapHirInput<'a> {
    fn try_new(
        sources: &'a CurrentConeParsedSources,
        authority: &'a CoreBootstrapAuthority,
        artifact_slot: &'a TrustedCoreArtifactSlot,
    ) -> Result<Self, CoreBootstrapHirInputError> {
        if authority.expected_identity() != scoop_identity::ConeIdentity::CORE {
            return Err(CoreBootstrapHirInputError::InvalidAuthorityIdentity(
                authority.expected_identity(),
            ));
        }
        if authority.artifact_path() != artifact_slot.path()
            || authority.target() != artifact_slot.target()
            || authority.toolchain_compatibility() != artifact_slot.toolchain_compatibility()
        {
            return Err(CoreBootstrapHirInputError::ArtifactSlotMismatch);
        }
        let sources = scoop_hir_lower::CoreBootstrapSources::try_new(sources)
            .map_err(CoreBootstrapHirInputError::Sources)?;
        Ok(Self {
            sources,
            authority,
            artifact_slot,
        })
    }

    pub fn lower(&self) -> Result<TrustedCoreBootstrapHirOutput, CoreBootstrapHirStageError> {
        let hir = scoop_hir_lower::lower_core_bootstrap(&self.sources)
            .map_err(CoreBootstrapHirStageError::Lowering)?;
        let production_section =
            scoop_hir::CoreBootstrapInterfaceSectionV1::from_export(&hir.export)
                .map_err(CoreBootstrapHirStageError::ProductionSection)?;
        Ok(TrustedCoreBootstrapHirOutput {
            hir,
            production_section,
        })
    }

    pub const fn authority(&self) -> &'a CoreBootstrapAuthority {
        self.authority
    }

    pub const fn artifact_slot(&self) -> &'a TrustedCoreArtifactSlot {
        self.artifact_slot
    }
}

/// Atomic trusted-core HIR product for the single-Cone production pipeline.
///
/// The private fields prevent downstream orchestration from omitting the
/// mandatory production section. The graph already owns its output contract,
/// and both are derived during the same successful stage.
pub struct TrustedCoreBootstrapHirOutput {
    hir: scoop_hir::Output,
    production_section: scoop_hir::CoreBootstrapInterfaceSectionV1,
}

impl TrustedCoreBootstrapHirOutput {
    pub const fn hir(&self) -> &scoop_hir::Output {
        &self.hir
    }

    pub const fn output_kind(&self) -> &scoop_hir::ConeOutputKind {
        self.hir.output_kind()
    }

    pub const fn production_section(&self) -> &scoop_hir::CoreBootstrapInterfaceSectionV1 {
        &self.production_section
    }
}

#[derive(Debug)]
pub enum CoreBootstrapHirStageError {
    Lowering(Vec<scoop_ast::Diagnostic>),
    ProductionSection(scoop_hir::CoreBootstrapInterfaceBuildError),
}

impl fmt::Display for CoreBootstrapHirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lowering(diagnostics) => write!(
                formatter,
                "trusted core HIR lowering failed with {} diagnostic(s)",
                diagnostics.len()
            ),
            Self::ProductionSection(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapHirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lowering(_) => None,
            Self::ProductionSection(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum CoreBootstrapHirInputError {
    InvalidAuthorityIdentity(scoop_identity::ConeIdentity),
    ArtifactSlotMismatch,
    Sources(scoop_hir_lower::CoreBootstrapSourceError),
}

impl fmt::Display for CoreBootstrapHirInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAuthorityIdentity(actual) => write!(
                formatter,
                "trusted core bootstrap authority names Cone {actual}, expected the reserved core Cone"
            ),
            Self::ArtifactSlotMismatch => formatter.write_str(
                "trusted core bootstrap authority and configured artifact slot do not match",
            ),
            Self::Sources(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapHirInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sources(source) => Some(source),
            Self::InvalidAuthorityIdentity(_) | Self::ArtifactSlotMismatch => None,
        }
    }
}

fn parse_manifest_current(
    manifest: &LoadedConeManifest,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    let sources = discover_manifest_sources(manifest)
        .map_err(|source| CurrentConeSourceStageError::Discovery(Box::new(source)))?;
    parse_discovered_sources(&sources)
}

fn parse_single_file_current(
    source: &SingleFileLocator,
) -> Result<CurrentConeParsedSources, CurrentConeSourceStageError> {
    let source = load_single_file_source(source)
        .map_err(|source| CurrentConeSourceStageError::SingleFile(Box::new(source)))?;
    parse_single_discovered_source(&source)
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

        let LoadedCurrentConeInput::Manifest { manifest } = &current else {
            panic!("test constructs a manifest input")
        };
        let parsed = parse_manifest_current(manifest).unwrap();

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

        let LoadedCurrentConeInput::SingleFile { source } = &current else {
            panic!("test constructs a single-file input")
        };
        let parsed = parse_single_file_current(source).unwrap();

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

        let LoadedCurrentConeInput::Manifest { manifest } = &current else {
            panic!("test constructs a manifest input")
        };
        let error = parse_manifest_current(manifest).unwrap_err();

        assert!(matches!(
            error,
            CurrentConeSourceStageError::Parser(source)
                if matches!(source.as_ref(), ParseCurrentConeError::Diagnostics(diagnostics)
                    if diagnostics.len() == 1
                        && diagnostics[0].identity().logical_path().as_str() == "src/bad.scoop")
        ));
    }

    #[test]
    fn real_trusted_core_sources_form_the_bootstrap_hir_interface() {
        let slot = crate::trusted_core::resolve_trusted_core_slot_at(
            &crate::workspace_root().join("sysroot"),
            scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        )
        .unwrap();
        let (bootstrap, artifact_slot) = slot.into_bootstrap_parts();
        let sources = discover_manifest_sources(bootstrap.source_slot().manifest()).unwrap();
        let parsed = parse_discovered_sources(&sources).unwrap();
        let input =
            TrustedCoreBootstrapHirInput::try_new(&parsed, bootstrap.authority(), &artifact_slot)
                .unwrap();

        assert!(std::ptr::eq(input.authority(), bootstrap.authority()));
        assert_eq!(input.artifact_slot(), &artifact_slot);
        let output = input.lower().unwrap();
        assert!(matches!(
            output.output_kind(),
            scoop_hir::ConeOutputKind::Library
        ));
        assert_eq!(
            output.production_section().output_contract(),
            &scoop_hir::HirOutputContractV1::Library
        );
        assert!(matches!(
            output.production_section().core_interface(),
            scoop_hir::CoreHirInterfaceBranchV1::Core(_)
        ));
        scoop_hir::CoreHirInterfaceV1::from_core_export(&output.hir().export).unwrap();

        let scoop_hir::CoreHirInterfaceBranchV1::Core(interface) =
            output.production_section().core_interface()
        else {
            panic!("the trusted bootstrap output has a core interface")
        };
        let signatures = interface
            .callable_targets()
            .targets()
            .iter()
            .filter_map(|target| {
                let scoop_hir::CoreHirCallableCapabilityV1::ParamFreeCandidate(signature) =
                    target.capability()
                else {
                    return None;
                };
                let scoop_hir::CoreCallableDefinitionV1::Function(definition) = target.definition()
                else {
                    panic!("a param-free core callable candidate has a source function definition")
                };
                Some(scoop_mir::CallableSignatureRecord::new(
                    scoop_mir::CallableSignatureSubject::Strong(
                        scoop_identity::CallableOwner::Function(definition),
                    ),
                    signature.clone(),
                ))
            })
            .collect::<Vec<_>>();
        assert!(!signatures.is_empty());

        let empty_mir_foundation =
            scoop_mir::OdrFreeMirFoundation::try_new(scoop_mir::CanonicalMirFoundation::empty())
                .unwrap();
        let empty_production = scoop_mir_lower::lower_production_section(
            scoop_identity::ConeIdentity::CORE,
            output.production_section(),
            &empty_mir_foundation,
        )
        .unwrap();
        let scoop_mir::CoreMirBridgeBranchV1::Core(empty_core_bridge) =
            empty_production.core_bridge()
        else {
            panic!("the trusted bootstrap MIR production has a core bridge")
        };
        assert!(empty_core_bridge.callable_targets().is_empty());

        let mut partial_foundation = scoop_mir::CanonicalMirFoundation::empty();
        partial_foundation
            .set_callable_signatures(vec![signatures[0].clone()])
            .unwrap();
        let partial_foundation =
            scoop_mir::OdrFreeMirFoundation::try_new(partial_foundation).unwrap();
        let partial_production = scoop_mir_lower::lower_production_section(
            scoop_identity::ConeIdentity::CORE,
            output.production_section(),
            &partial_foundation,
        )
        .unwrap();
        let scoop_mir::CoreMirBridgeBranchV1::Core(partial_core_bridge) =
            partial_production.core_bridge()
        else {
            panic!("the trusted bootstrap MIR production has a core bridge")
        };
        assert_eq!(partial_core_bridge.callable_targets().len(), 1);

        let mut mismatched_signatures = signatures.clone();
        let expected = mismatched_signatures[0].signature();
        let wrong_effect = match expected.effect() {
            scoop_identity::Effect::Ordinary => scoop_identity::Effect::Suspend,
            scoop_identity::Effect::Suspend => scoop_identity::Effect::Ordinary,
        };
        mismatched_signatures[0] = scoop_mir::CallableSignatureRecord::new(
            mismatched_signatures[0].subject(),
            scoop_identity::ExactCallableSignature::new(
                wrong_effect,
                expected.receiver().into_option(),
                expected.parameters().to_vec(),
                expected.result(),
            ),
        );
        let mut mismatched_foundation = scoop_mir::CanonicalMirFoundation::empty();
        mismatched_foundation
            .set_callable_signatures(mismatched_signatures)
            .unwrap();
        let mismatched_foundation =
            scoop_mir::OdrFreeMirFoundation::try_new(mismatched_foundation).unwrap();
        assert!(matches!(
            scoop_mir_lower::lower_production_section(
                scoop_identity::ConeIdentity::CORE,
                output.production_section(),
                &mismatched_foundation,
            ),
            Err(scoop_mir_lower::MirProductionLoweringError::CoreCallableSignatureMismatch { .. })
        ));

        let mut mir_foundation = scoop_mir::CanonicalMirFoundation::empty();
        mir_foundation.set_callable_signatures(signatures).unwrap();
        let mir_foundation = scoop_mir::OdrFreeMirFoundation::try_new(mir_foundation).unwrap();
        assert!(matches!(
            scoop_mir_lower::lower_production_section(
                scoop_identity::ConeIdentity::SINGLE_FILE,
                output.production_section(),
                &mir_foundation,
            ),
            Err(scoop_mir_lower::MirProductionLoweringError::Production(
                scoop_mir::MirProductionBuildError::UnexpectedCoreBridge(
                    scoop_identity::ConeIdentity::SINGLE_FILE
                )
            ))
        ));
        let mir_production = scoop_mir_lower::lower_production_section(
            scoop_identity::ConeIdentity::CORE,
            output.production_section(),
            &mir_foundation,
        )
        .unwrap();
        let scoop_mir::CoreMirBridgeBranchV1::Core(core_bridge) = mir_production.core_bridge()
        else {
            panic!("the trusted bootstrap MIR production has a core bridge")
        };
        assert_eq!(
            core_bridge.callable_targets().len(),
            interface
                .callable_targets()
                .targets()
                .iter()
                .filter(|target| matches!(
                    target.capability(),
                    scoop_hir::CoreHirCallableCapabilityV1::ParamFreeCandidate(_)
                ))
                .count()
        );
    }

    #[test]
    fn bootstrap_hir_input_rejects_an_artifact_slot_from_another_sysroot() {
        let slot = crate::trusted_core::resolve_trusted_core_slot_at(
            &crate::workspace_root().join("sysroot"),
            scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        )
        .unwrap();
        let (bootstrap, _) = slot.into_bootstrap_parts();
        let sources = discover_manifest_sources(bootstrap.source_slot().manifest()).unwrap();
        let parsed = parse_discovered_sources(&sources).unwrap();

        let other = tempfile::tempdir().unwrap();
        let other_core = other.path().join("lib/scoop.core");
        std::fs::create_dir_all(&other_core).unwrap();
        std::fs::write(
            other_core.join("Cone.toml"),
            "schema = 1\n[cone]\ngroup = \"scoop\"\nname = \"scoop.core\"\nversion = \"0.1.0\"\nkind = \"library\"\n",
        )
        .unwrap();
        let other_slot = crate::trusted_core::resolve_trusted_core_slot_at(
            other.path(),
            scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        )
        .unwrap();

        assert!(matches!(
            TrustedCoreBootstrapHirInput::try_new(
                &parsed,
                bootstrap.authority(),
                other_slot.artifact(),
            ),
            Err(CoreBootstrapHirInputError::ArtifactSlotMismatch)
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
