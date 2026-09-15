use std::fmt;
use std::path::{Path, PathBuf};

use scoop_ast::{CurrentConeParsedSources, NonEmptyVec};
use scoop_hir::CoreInterfaceImportError;
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
    CoreBootstrapAuthority, LoadedTrustedCoreArtifact, SingleConeStrongIrProductionV1,
    TrustedCoreArtifactLoadError, TrustedCoreArtifactSlot, TrustedCoreArtifactValidationError,
    TrustedCoreBootstrapInput, ValidatedTrustedCoreArtifact,
};

mod ordinary;
pub use ordinary::{
    OrdinaryConeHirOutput, OrdinaryConeHirStageError, OrdinaryConeLirOutput,
    OrdinaryConeLirStageError, OrdinaryConeMirOutput, OrdinaryConeMirStageError,
    OrdinaryConeStrongProfileError,
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
        let core = self
            .trusted_core
            .import_core_inputs()
            .map_err(OrdinaryCoreOnlyHirInputError::CoreInterface)?;
        scoop_hir_lower::OrdinaryCoreOnlySources::try_new(&self.sources, core)
            .map_err(OrdinaryCoreOnlyHirInputError::Sources)
    }
}

#[derive(Debug)]
pub enum OrdinaryCoreOnlyHirInputError {
    CoreInterface(CoreInterfaceImportError),
    Sources(scoop_hir_lower::OrdinaryCoreOnlySourceError),
}

impl fmt::Display for OrdinaryCoreOnlyHirInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoreInterface(source) => source.fmt(formatter),
            Self::Sources(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for OrdinaryCoreOnlyHirInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CoreInterface(source) => Some(source),
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

    /// Consumes the complete parsed bootstrap request through the shared
    /// strong object pipeline and atomically publishes the independently
    /// revalidated artifact to its trusted sysroot slot.
    pub fn build_and_publish(
        self,
        temporary_parent: &Path,
        limits: DecodeLimits,
    ) -> Result<scoop_slib::PublishedSingleConeArtifact, CoreBootstrapProductionError> {
        let lir = self
            .hir_input()
            .map_err(CoreBootstrapProductionError::HirInput)?
            .lower()
            .map_err(CoreBootstrapProductionError::Hir)?
            .lower_mir()
            .map_err(CoreBootstrapProductionError::Mir)?
            .lower_lir(self.request.target().lir_target())
            .map_err(CoreBootstrapProductionError::Lir)?;
        let strong = lir
            .seal_strong_profile()
            .map_err(CoreBootstrapProductionError::StrongProfile)?;
        let producer =
            scoop_slib::ProducerRecord::new(concat!("scoopc/", env!("CARGO_PKG_VERSION")))
                .map_err(CoreBootstrapProductionError::Producer)?;
        let cone = scoop_slib::ConeRecord::new(
            ConeCoordinate::reserved_core(),
            scoop_slib::ConeKind::Library,
            scoop_slib::ConeSourceForm::Manifest,
        )
        .map_err(CoreBootstrapProductionError::Cone)?;
        let core_owners = scoop_slib::CanonicalDefinedLinkSymbolOwnerSetV1::empty_core_bootstrap();
        let artifact = strong
            .produce_artifact(
                producer,
                cone,
                Vec::new(),
                temporary_parent,
                self.request.target(),
                &core_owners,
            )
            .map_err(CoreBootstrapProductionError::Artifact)?;
        artifact
            .publish(self.artifact_slot.path(), limits, &core_owners)
            .map_err(CoreBootstrapProductionError::Publication)
    }
}

#[derive(Debug)]
pub enum CoreBootstrapProductionError {
    HirInput(CoreBootstrapHirInputError),
    Hir(CoreBootstrapHirStageError),
    Mir(CoreBootstrapMirStageError),
    Lir(CoreBootstrapLirStageError),
    StrongProfile(CoreBootstrapStrongProfileError),
    Producer(scoop_slib::ProducerRecordError),
    Cone(scoop_slib::ConeRecordError),
    Artifact(crate::StrongIrArtifactProductionError),
    Publication(crate::StrongArtifactProductionError),
}

impl fmt::Display for CoreBootstrapProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HirInput(source) => source.fmt(formatter),
            Self::Hir(source) => source.fmt(formatter),
            Self::Mir(source) => source.fmt(formatter),
            Self::Lir(source) => source.fmt(formatter),
            Self::StrongProfile(source) => source.fmt(formatter),
            Self::Producer(source) => source.fmt(formatter),
            Self::Cone(source) => source.fmt(formatter),
            Self::Artifact(source) => source.fmt(formatter),
            Self::Publication(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HirInput(source) => Some(source),
            Self::Hir(source) => Some(source),
            Self::Mir(source) => Some(source),
            Self::Lir(source) => Some(source),
            Self::StrongProfile(source) => Some(source),
            Self::Producer(source) => Some(source),
            Self::Cone(source) => Some(source),
            Self::Artifact(source) => Some(source),
            Self::Publication(source) => Some(source),
        }
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
        let foundation = scoop_hir::CanonicalHirFoundation::from_modules(
            &hir.export,
            &hir.local,
            &hir.native_boundary_types,
        )
        .map_err(CoreBootstrapHirStageError::Foundation)?;
        let production_section =
            scoop_hir::CoreBootstrapInterfaceSectionV1::from_export(&hir.export)
                .map_err(CoreBootstrapHirStageError::ProductionSection)?;
        Ok(TrustedCoreBootstrapHirOutput {
            hir,
            foundation,
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
    foundation: scoop_hir::CanonicalHirFoundation,
    production_section: scoop_hir::CoreBootstrapInterfaceSectionV1,
}

impl TrustedCoreBootstrapHirOutput {
    pub const fn hir(&self) -> &scoop_hir::Output {
        &self.hir
    }

    pub const fn output_kind(&self) -> &scoop_hir::ConeOutputKind {
        self.hir.output_kind()
    }

    pub const fn foundation(&self) -> &scoop_hir::CanonicalHirFoundation {
        &self.foundation
    }

    pub const fn production_section(&self) -> &scoop_hir::CoreBootstrapInterfaceSectionV1 {
        &self.production_section
    }

    /// Advances this exact sealed HIR product through MIR lowering.
    ///
    /// Consuming `self` keeps the HIR graph, its mandatory production
    /// section, the MIR graph, and the derived ODR-free foundation in one
    /// inseparable stage product.
    pub fn lower_mir(self) -> Result<TrustedCoreBootstrapMirOutput, CoreBootstrapMirStageError> {
        let scoop_hir::LocalConcreteMaterializationContract::CoreShapeSupport(shape_support) =
            self.hir.local.materialization()
        else {
            return Err(CoreBootstrapMirStageError::MissingCoreShapeSupportPlan);
        };
        let shape_sources = scoop_mir::CoreShapeSupportSourceInput::Core(
            shape_support
                .roots()
                .iter()
                .map(|root| root.declaration().clone())
                .collect(),
        );
        let mir = scoop_mir_lower::lower(&self.hir.local)
            .map_err(CoreBootstrapMirStageError::Lowering)?;
        let foundation = scoop_mir::OdrFreeMirFoundation::from_module(&mir)
            .map_err(CoreBootstrapMirStageError::Foundation)?;
        let production_section = scoop_mir_lower::lower_production_section(
            mir.cone,
            &self.production_section,
            &foundation,
        )
        .map_err(CoreBootstrapMirStageError::ProductionSection)?;
        let strong = scoop_mir::SingleConeStrongMirInput::try_new(
            mir,
            foundation,
            production_section,
            shape_sources,
            scoop_mir::StrongImportedCoreInput::Unused,
        )
        .map_err(CoreBootstrapMirStageError::Sealing)?;
        Ok(TrustedCoreBootstrapMirOutput { hir: self, strong })
    }
}

/// Atomic trusted-core MIR product for the single-Cone strong pipeline.
///
/// The previous HIR stage is owned rather than referenced so no caller can
/// pair this MIR graph or production section with a different HIR proof.
pub struct TrustedCoreBootstrapMirOutput {
    hir: TrustedCoreBootstrapHirOutput,
    strong: scoop_mir::SingleConeStrongMirInput,
}

impl TrustedCoreBootstrapMirOutput {
    pub const fn hir(&self) -> &TrustedCoreBootstrapHirOutput {
        &self.hir
    }

    pub const fn mir(&self) -> &scoop_mir::Module {
        self.strong.module()
    }

    pub const fn foundation(&self) -> &scoop_mir::OdrFreeMirFoundation {
        self.strong.foundation()
    }

    pub const fn production_section(&self) -> &scoop_mir::CoreBootstrapBridgeSectionV1 {
        self.strong.production()
    }

    pub const fn materialization_plan(&self) -> &scoop_mir::SingleConeStrongMaterializationPlan {
        self.strong.materialization()
    }

    /// Advances this exact sealed MIR product through strong LIR lowering.
    ///
    /// Consuming `self` keeps the complete bootstrap proof chain attached to
    /// the resulting ODR-free LIR graph.
    pub fn lower_lir(
        self,
        target_profile: scoop_lir::LirTargetProfile,
    ) -> Result<TrustedCoreBootstrapLirOutput, CoreBootstrapLirStageError> {
        let lir = scoop_lir_lower::lower(
            &self.strong,
            scoop_lir_lower::StrongImportedCoreLirInput::Unused,
            target_profile,
        )
        .map_err(CoreBootstrapLirStageError::Lowering)?;
        Ok(TrustedCoreBootstrapLirOutput { mir: self, lir })
    }
}

/// Atomic trusted-core LIR product for the single-Cone strong pipeline.
///
/// Its LIR graph is inseparable from the strong MIR input that selected every
/// persistent materialization root, and from the projected ODR-free
/// foundation that proves lowering did not introduce an ODR-owned entity.
pub struct TrustedCoreBootstrapLirOutput {
    mir: TrustedCoreBootstrapMirOutput,
    lir: scoop_lir::SingleConeStrongLirOutput,
}

impl TrustedCoreBootstrapLirOutput {
    pub const fn mir_stage(&self) -> &TrustedCoreBootstrapMirOutput {
        &self.mir
    }

    pub const fn lir(&self) -> &scoop_lir::Module {
        self.lir.module()
    }

    pub const fn foundation(&self) -> &scoop_lir::OdrFreeLirFoundation {
        self.lir.foundation()
    }

    pub const fn core_shape_support(&self) -> &scoop_lir::StrongLirCoreShapeSupportPlan {
        self.lir.core_shape_support()
    }

    /// Seals all three IR foundations under the strong profile's `RejectAll`
    /// policy before object production can observe this lowering result.
    pub fn seal_strong_profile(
        self,
    ) -> Result<SingleConeStrongIrProductionV1, CoreBootstrapStrongProfileError> {
        let hir_foundation =
            scoop_hir::OdrFreeHirFoundation::try_new(self.mir.hir.foundation.clone())
                .map_err(CoreBootstrapStrongProfileError::HirOdr)?;
        let Self { mir, lir } = self;
        let TrustedCoreBootstrapMirOutput { hir, strong } = mir;
        Ok(SingleConeStrongIrProductionV1::new(
            hir_foundation,
            hir.production_section,
            strong.foundation().clone(),
            strong.production().clone(),
            lir,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreBootstrapStrongProfileError {
    HirOdr(scoop_hir::OdrFreeHirFoundationError),
}

impl fmt::Display for CoreBootstrapStrongProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HirOdr(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapStrongProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HirOdr(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum CoreBootstrapLirStageError {
    Lowering(scoop_lir_lower::StrongLirLoweringError),
}

impl fmt::Display for CoreBootstrapLirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lowering(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapLirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lowering(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum CoreBootstrapMirStageError {
    MissingCoreShapeSupportPlan,
    Lowering(scoop_mir_lower::DefinedCoreMirLoweringError),
    Foundation(scoop_mir::OdrFreeMirFoundationProjectionError),
    ProductionSection(scoop_mir_lower::MirProductionLoweringError),
    Sealing(scoop_mir::SingleConeStrongMirInputError),
}

impl fmt::Display for CoreBootstrapMirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCoreShapeSupportPlan => {
                formatter.write_str("trusted core HIR output has no core shape-support plan")
            }
            Self::Lowering(source) => source.fmt(formatter),
            Self::Foundation(source) => source.fmt(formatter),
            Self::ProductionSection(source) => source.fmt(formatter),
            Self::Sealing(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapMirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::MissingCoreShapeSupportPlan => return None,
            Self::Lowering(source) => source,
            Self::Foundation(source) => source,
            Self::ProductionSection(source) => source,
            Self::Sealing(source) => source,
        })
    }
}

#[derive(Debug)]
pub enum CoreBootstrapHirStageError {
    Lowering(Vec<scoop_ast::Diagnostic>),
    Foundation(scoop_hir::HirFoundationBuildError),
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
            Self::Foundation(source) => source.fmt(formatter),
            Self::ProductionSection(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreBootstrapHirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lowering(_) => None,
            Self::Foundation(source) => Some(source),
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
        assert_eq!(
            output.foundation(),
            &scoop_hir::CanonicalHirFoundation::from_modules(
                &output.hir().export,
                &output.hir().local,
                &output.hir().native_boundary_types,
            )
            .unwrap()
        );
        let production_bytes = scoop_wire::encode(output.production_section()).unwrap();
        let decoded = scoop_wire::decode_canonical::<
            scoop_hir::DecodedCoreBootstrapInterfaceSectionV1,
        >(&production_bytes, scoop_wire::DecodeLimits::default())
        .unwrap();
        let odr_free_foundation =
            scoop_hir::OdrFreeHirFoundation::try_new(output.foundation().clone()).unwrap();
        assert_eq!(
            decoded
                .validate_against_strong_foundation(
                    scoop_identity::ConeIdentity::CORE,
                    &odr_free_foundation,
                )
                .unwrap(),
            output.production_section().clone()
        );
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

        let candidate_count = interface
            .callable_targets()
            .targets()
            .iter()
            .filter(|target| {
                matches!(
                    target.capability(),
                    scoop_hir::CoreHirCallableCapabilityV1::ParamFreeCandidate(_)
                )
            })
            .count();
        let expected_shape_roots = interface
            .type_targets()
            .targets()
            .iter()
            .filter(|target| {
                matches!(
                    (target.definition(), target.capability()),
                    (
                        scoop_hir::CoreTypeDefinitionV1::Type(_),
                        scoop_hir::CoreHirTypeCapabilityV1::ParamFreeStrong(_)
                    )
                )
            })
            .count();
        let missing_source_module = scoop_mir_lower::lower(&output.hir().local).unwrap();
        let missing_source_foundation =
            scoop_mir::OdrFreeMirFoundation::from_module(&missing_source_module).unwrap();
        let missing_source_production = scoop_mir_lower::lower_production_section(
            missing_source_module.cone,
            output.production_section(),
            &missing_source_foundation,
        )
        .unwrap();
        assert!(matches!(
            scoop_mir::SingleConeStrongMirInput::try_new(
                missing_source_module,
                missing_source_foundation,
                missing_source_production,
                scoop_mir::CoreShapeSupportSourceInput::NotCore,
                scoop_mir::StrongImportedCoreInput::Unused,
            ),
            Err(scoop_mir::SingleConeStrongMirInputError::CoreShapeSupportSourceBranchMismatch)
        ));
        let real_mir = output.lower_mir().unwrap();
        assert_eq!(real_mir.mir().cone, scoop_identity::ConeIdentity::CORE);
        let scoop_hir::LocalConcreteMaterializationContract::CoreShapeSupport(shape_plan) =
            real_mir.hir().hir().local.materialization()
        else {
            panic!("trusted core MIR retains the authoritative local shape plan")
        };
        assert_eq!(shape_plan.roots().len(), expected_shape_roots);
        for root in shape_plan.roots() {
            let exact = root.exact();
            let mir = real_mir.mir();
            assert!(
                mir.meta
                    .coroutine_steps
                    .iter()
                    .any(|(_, step)| { step.identity().result_record().id() == exact })
            );
            assert!(
                mir.meta
                    .coroutine_slots
                    .iter()
                    .any(|(_, slot)| { slot.identity().value_record().id() == exact })
            );
            let boxed = mir.meta.boxed_types.iter().any(|boxed| {
                matches!(
                    boxed.identity().generated_type_record().key(),
                    scoop_identity::GeneratedNominalKey::BoxedValue { payload }
                        if *payload == exact
                )
            });
            assert_eq!(
                boxed,
                root.boxed_value() == scoop_hir::LocalCoreBoxedValueRequirement::Required
            );
        }
        let scoop_mir::CoreMirBridgeBranchV1::Core(real_core_bridge) =
            real_mir.production_section().core_bridge()
        else {
            panic!("the real trusted bootstrap MIR product has a core bridge")
        };
        assert!(candidate_count > 0);
        assert!(real_core_bridge.callable_targets().is_empty());
        assert!(expected_shape_roots > 0);
        assert_eq!(
            real_core_bridge.shape_support_roots().len(),
            expected_shape_roots
        );
        assert_eq!(
            real_mir.materialization_plan().callable_roots().len(),
            real_mir.mir().top_level.len()
        );
        assert_eq!(
            real_mir.materialization_plan().core_shape_support_roots(),
            real_core_bridge.shape_support_roots()
        );
        assert_eq!(
            real_mir.materialization_plan().core_shape_support_sources(),
            shape_plan
                .roots()
                .iter()
                .map(|root| root.declaration().clone())
                .collect::<Vec<_>>()
        );
        assert!(
            !real_mir
                .materialization_plan()
                .source_nominal_shapes()
                .is_empty()
        );
        assert_eq!(
            real_mir
                .materialization_plan()
                .generated_nominal_shapes()
                .len(),
            real_mir.mir().meta.generated_exact_types.len()
        );
        assert_eq!(
            real_mir
                .production_section()
                .strong_callable_bridges()
                .bridges()
                .len(),
            real_mir.mir().meta.callable_signatures.len()
        );
        let expected_lir_shape_roots = real_core_bridge.shape_support_roots().to_vec();
        let real_lir = real_mir
            .lower_lir(scoop_lir::LirTargetProfile::DARWIN_AARCH64)
            .unwrap();
        let scoop_lir::StrongLirCoreShapeSupportPlan::Core(lir_shape_roots) =
            real_lir.core_shape_support()
        else {
            panic!("trusted core LIR retains its validated core shape plan")
        };
        assert_eq!(lir_shape_roots.len(), expected_shape_roots);
        for (root, authority) in lir_shape_roots.iter().zip(&expected_lir_shape_roots) {
            assert_eq!(
                root.source().nominal(),
                scoop_identity::PersistentTypeId::from_source_declaration(root.declaration())
                    .unwrap()
            );
            assert_eq!(root.source().nominal(), authority.source());
            assert_eq!(root.source().exact(), authority.exact());
            assert_eq!(root.source().type_descriptor(), root.source().exact());
            assert_eq!(
                root.coroutine_step().type_descriptor(),
                root.coroutine_step().exact()
            );
            assert_eq!(
                root.coroutine_slot().type_descriptor(),
                root.coroutine_slot().exact()
            );
            if let scoop_lir::StrongLirBoxedValueMaterialization::Available(boxed) =
                root.boxed_value()
            {
                assert_eq!(boxed.type_descriptor(), boxed.exact());
            }
        }
        let lir_counts = real_lir.foundation().as_canonical().counts();
        assert_eq!(lir_counts.odr_groups, 0);
        assert_eq!(lir_counts.odr_members, 0);
        assert!(
            !real_lir
                .lir()
                .meta
                .layouts
                .iter()
                .any(|(_, layout)| layout.name.starts_with("Option<"))
        );
        assert!(
            !real_lir
                .lir()
                .meta
                .type_descriptors
                .iter()
                .any(
                    |(_, descriptor)| descriptor.diagnostic_name.starts_with("Iterable<")
                        || descriptor.diagnostic_name.starts_with("Iterator<")
                )
        );

        let production = real_lir
            .lir
            .build_production_section(
                scoop_identity::ConeCoordinate::reserved_core(),
                scoop_lir::EntryProductionSourceV1::Library,
            )
            .unwrap();
        let scoop_lir::CoreShapeSupportPlanV1::Core(production_shape_support) =
            production.core_shape_support()
        else {
            panic!("trusted core production retains its core shape-support branch")
        };
        assert_eq!(
            production_shape_support.closures().len(),
            expected_shape_roots
        );
        let strong = real_lir.seal_strong_profile().unwrap();
        let hir_counts = strong.hir_foundation().as_canonical().counts();
        assert_eq!(hir_counts.callable_applications, 0);
        assert_eq!(hir_counts.odr_groups, 0);
        assert_eq!(hir_counts.odr_members, 0);
    }

    #[test]
    fn parsed_bootstrap_request_publishes_one_two_view_core_artifact() {
        let sysroot = tempfile::tempdir().unwrap();
        copy_trusted_core_sources(sysroot.path());
        let Ok(target) = scoop_codegen::ResolvedTargetProfile::resolve_host() else {
            // The target resolver owns host Apple-toolchain qualification.
            // Environments blocked by the system Xcode license gate cannot
            // enter object production, but still compile this closed path.
            return;
        };
        let slot = crate::trusted_core::resolve_trusted_core_slot_at(
            sysroot.path(),
            target.lir_target_selection(),
        )
        .unwrap();
        let artifact_path = slot.artifact().path().to_path_buf();
        std::fs::create_dir_all(artifact_path.parent().unwrap()).unwrap();
        let (bootstrap, artifact_slot) = slot.into_bootstrap_parts();
        let request = SingleConeBuildRequest::new(
            CurrentConeInput::TrustedCoreBootstrap {
                input: Box::new(bootstrap),
            },
            ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
            TrustedCoreInput::BootstrapSelf { artifact_slot },
            target,
            SlibOutputDestination::new(&artifact_path).unwrap(),
            DiagnosticOutputPolicy::Human,
            StageDumpPolicy::None,
        )
        .unwrap();
        let loaded = request.load_preflight(DecodeLimits::default()).unwrap();
        let validated = loaded.validate().unwrap();
        let parsed = validated.parse_current_sources().unwrap();
        let ParsedSingleConeBuildRequest::TrustedCoreBootstrap(parsed) = parsed else {
            panic!("the request retains its trusted bootstrap branch")
        };

        let published = parsed
            .build_and_publish(&sysroot.path().join("temporary"), DecodeLimits::default())
            .unwrap();

        assert_eq!(published.path(), artifact_path);
        assert_eq!(
            published.validation().coordinate(),
            &ConeCoordinate::reserved_core()
        );
        assert_eq!(
            published.validation().identity(),
            scoop_identity::ConeIdentity::CORE
        );
        assert_eq!(published.validation().kind(), scoop_slib::ConeKind::Library);
        assert_eq!(
            published.validation().source_form(),
            scoop_slib::ConeSourceForm::Manifest
        );
        assert!(published.validation().link_summary().link_object_count() > 0);

        let ordinary_source = sysroot.path().join("ordinary.scoop");
        std::fs::write(&ordinary_source, "fun main() {}\n").unwrap();
        let target = scoop_codegen::ResolvedTargetProfile::resolve_host().unwrap();
        let core_slot = crate::trusted_core::resolve_trusted_core_slot_at(
            sysroot.path(),
            target.lir_target_selection(),
        )
        .unwrap();
        let ordinary_request = SingleConeBuildRequest::new(
            CurrentConeInput::SingleFile {
                source: SingleFileLocator::from_path(&ordinary_source).unwrap(),
            },
            ExplicitDependencyInputs::new(Vec::new(), Vec::new()).unwrap(),
            TrustedCoreInput::Artifact(core_slot.existing_artifact_input().unwrap()),
            target,
            SlibOutputDestination::new(sysroot.path().join("ordinary.slib")).unwrap(),
            DiagnosticOutputPolicy::Human,
            StageDumpPolicy::None,
        )
        .unwrap();
        let ordinary_loaded = ordinary_request
            .load_preflight(DecodeLimits::default())
            .unwrap();
        let ordinary_validated = ordinary_loaded.validate().unwrap();
        let ordinary_parsed = ordinary_validated.parse_current_sources().unwrap();
        let ParsedSingleConeBuildRequest::Ordinary(ordinary_parsed) = ordinary_parsed else {
            panic!("the single-file request retains its ordinary branch")
        };
        let ordinary_strong = ordinary_parsed
            .lower_hir()
            .unwrap()
            .lower_mir()
            .unwrap()
            .lower_lir()
            .unwrap()
            .seal_strong_profile()
            .unwrap();

        assert_eq!(
            ordinary_strong.lir_output().module().cone,
            scoop_identity::ConeIdentity::SINGLE_FILE
        );
        assert!(matches!(
            ordinary_strong.hir_production().core_interface(),
            scoop_hir::CoreHirInterfaceBranchV1::NotCore
        ));
        assert!(matches!(
            ordinary_strong.mir_production().core_bridge(),
            scoop_mir::CoreMirBridgeBranchV1::NotCore
        ));
        assert!(
            ordinary_strong
                .lir_output()
                .module()
                .meta
                .core_external_callables
                .is_empty()
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

    fn copy_trusted_core_sources(sysroot: &Path) {
        let source = crate::workspace_root().join("sysroot/lib/scoop.core");
        let destination = sysroot.join("lib/scoop.core");
        std::fs::create_dir_all(destination.join("src")).unwrap();
        std::fs::copy(source.join("Cone.toml"), destination.join("Cone.toml")).unwrap();
        for entry in std::fs::read_dir(source.join("src")).unwrap() {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            std::fs::copy(
                entry.path(),
                destination.join("src").join(entry.file_name()),
            )
            .unwrap();
        }
    }
}
