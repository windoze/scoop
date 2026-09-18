use std::fmt;
use std::path::{Path, PathBuf};

use scoop_ast::{CurrentConeParsedSources, NonEmptyVec};
use scoop_hir::CoreInterfaceImportError;
use scoop_identity::{ConeCoordinate, ConeIdentity};
use scoop_manifest::{
    DiscoveredManifestSources, DiscoveredSource, LoadedConeManifest, ManifestRootError,
    ManifestSpan, SingleFileInputError, SingleFileInputErrorKind, SingleFileLocator,
    SourceDiscoveryError, SourceDiscoveryErrorKind, discover_manifest_sources, load_cone_manifest,
    load_single_file_source,
};
use scoop_parser::{CurrentConeSourceInput, ParseCurrentConeError, parse_current_cone};
use scoop_slib::{PrebuiltManifestSummaryError, SlibClosureResourceErrorV1};
#[cfg(test)]
use scoop_slib::{SlibClosureDecodeLimitsV1, SlibClosureDecodeMeterV1};
use scoop_wire::DecodeLimits;

use super::{
    CurrentConeDiagnosticSet, CurrentConeDiagnosticSetError, CurrentConeInput,
    DiagnosticOutputPolicy, EmittedStageDump, SingleConeBuildRequest, SingleConeProductionSuccess,
    SlibOutputDestination, StageDumpKind, StageDumpPolicy, TrustedCoreInput,
};
use crate::{
    CoreBootstrapAuthority, LoadedTrustedCoreArtifact, SingleConeStrongIrProductionV1,
    TrustedCoreArtifactLoadError, TrustedCoreArtifactSlot, TrustedCoreArtifactValidationError,
    TrustedCoreBootstrapInput, ValidatedTrustedCoreArtifact,
};

mod dependencies;
#[cfg(test)]
mod end_to_end_tests;
mod metering;
mod ordinary;
use dependencies::LoadedExplicitDependencyInputs;
pub use dependencies::{
    ExplicitDependencyArtifactInput, ExplicitDependencyLoadError, ExplicitDependencyLoadOperation,
    ExplicitDependencyRole, ExplicitDependencyValidationError,
};
pub use ordinary::{
    OrdinaryConeHirOutput, OrdinaryConeHirStageError, OrdinaryConeLirOutput,
    OrdinaryConeLirStageError, OrdinaryConeMirOutput, OrdinaryConeMirStageError,
    OrdinaryConeProductionError, OrdinaryConeStrongProfileError,
};

/// Proof that the manifest, explicit artifacts, and their recursive closure
/// were all validated and contain no non-core dependency.
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
    dependencies: LoadedExplicitDependencyInputs,
    trusted_core: LoadedTrustedCoreInput,
    target: scoop_toolchain::ResolvedTargetProfile,
    output: SlibOutputDestination,
    diagnostics: DiagnosticOutputPolicy,
    emit: StageDumpPolicy,
}

impl SingleConeBuildRequest {
    /// Executes the only single-Cone production path and atomically publishes
    /// the resulting `.slib` artifact.
    pub fn build_and_publish(
        self,
        limits: DecodeLimits,
    ) -> Result<SingleConeProductionSuccess, SingleConeProductionError> {
        self.build_and_publish_with_closure_limits(limits, metering::production_closure_limits())
    }

    /// Loads the current manifest, every explicit dependency artifact, and
    /// trusted core bytes. Current source discovery and parsing remain
    /// deliberately unavailable at this stage.
    pub fn load_preflight(
        self,
        limits: DecodeLimits,
    ) -> Result<LoadedSingleConeBuildRequest, SingleConePreflightError> {
        self.load_preflight_inner(limits, None)
    }
}

fn capture_stage_dump(
    policy: StageDumpPolicy,
    kind: StageDumpKind,
    render: impl FnOnce() -> String,
) -> Option<EmittedStageDump> {
    if policy == StageDumpPolicy::Stage(kind) {
        Some(EmittedStageDump::new(kind, render()))
    } else {
        None
    }
}

#[derive(Debug)]
pub enum SingleConeProductionError {
    TemporaryWorkspace(std::io::Error),
    Preflight(SingleConePreflightError),
    Validation(CoreOnlyRequestValidationError),
    Sources(CurrentConeSourceStageError),
    Ordinary(OrdinaryConeProductionError),
    CoreBootstrap(CoreBootstrapProductionError),
}

impl fmt::Display for SingleConeProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TemporaryWorkspace(source) => {
                write!(
                    formatter,
                    "cannot create temporary build workspace: {source}"
                )
            }
            Self::Preflight(source) => source.fmt(formatter),
            Self::Validation(source) => source.fmt(formatter),
            Self::Sources(source) => source.fmt(formatter),
            Self::Ordinary(source) => source.fmt(formatter),
            Self::CoreBootstrap(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for SingleConeProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::TemporaryWorkspace(source) => source,
            Self::Preflight(source) => source,
            Self::Validation(source) => source,
            Self::Sources(source) => source,
            Self::Ordinary(source) => source,
            Self::CoreBootstrap(source) => source,
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

impl LoadedSingleConeBuildRequest {
    /// Constructs the trusted-core proof before exposing any current source
    /// loading or parser entry.
    pub fn validate(
        &self,
    ) -> Result<ValidatedCoreOnlyBuildRequest<'_>, CoreOnlyRequestValidationError> {
        self.validate_inner(None)
    }
}

#[derive(Debug)]
pub enum CoreOnlyRequestValidationError {
    InvalidLoadedInputPair,
    CurrentIdentity(scoop_wire::HashError),
    Resource(SlibClosureResourceErrorV1),
    TrustedCoreSummary(Box<PrebuiltManifestSummaryError>),
    TrustedCore(Box<TrustedCoreArtifactValidationError>),
    ExplicitDependencies(Box<ExplicitDependencyValidationError>),
}

impl fmt::Display for CoreOnlyRequestValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLoadedInputPair => formatter
                .write_str("loaded current Cone and trusted core inputs are not a permitted pair"),
            Self::CurrentIdentity(source) => source.fmt(formatter),
            Self::Resource(source) => source.fmt(formatter),
            Self::TrustedCoreSummary(source) => source.fmt(formatter),
            Self::TrustedCore(source) => source.fmt(formatter),
            Self::ExplicitDependencies(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CoreOnlyRequestValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidLoadedInputPair => None,
            Self::CurrentIdentity(source) => Some(source),
            Self::Resource(source) => Some(source),
            Self::TrustedCoreSummary(source) => Some(source.as_ref()),
            Self::TrustedCore(source) => Some(source.as_ref()),
            Self::ExplicitDependencies(source) => Some(source.as_ref()),
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
    dependencies: ValidatedExplicitDependencyInputSet,
}

impl<'input> ValidatedCoreOnlyBuildRequest<'input> {
    pub const fn current(&self) -> &ValidatedCurrentConeInput<'input> {
        &self.current
    }

    pub const fn dependencies(&self) -> ValidatedExplicitDependencyInputSet {
        self.dependencies
    }

    pub const fn target(&self) -> &scoop_toolchain::ResolvedTargetProfile {
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
    ) -> Result<SingleConeProductionSuccess, CoreBootstrapProductionError> {
        let emit = self.request.emit();
        let mut emitted_dump = capture_stage_dump(emit, StageDumpKind::Ast, || {
            self.sources
                .sources()
                .sources()
                .iter()
                .map(|source| scoop_ast::dump(source.ast()))
                .collect()
        });
        let hir = self
            .hir_input()
            .map_err(CoreBootstrapProductionError::HirInput)?
            .lower()
            .map_err(CoreBootstrapProductionError::Hir)?;
        let warnings = CurrentConeDiagnosticSet::try_new(hir.hir().warnings.clone(), &self.sources)
            .map_err(CoreBootstrapProductionError::Warnings)?;
        emitted_dump = emitted_dump.or_else(|| {
            capture_stage_dump(emit, StageDumpKind::Hir, || {
                scoop_hir::dump(&hir.hir().export)
            })
        });
        let mir = hir.lower_mir().map_err(CoreBootstrapProductionError::Mir)?;
        emitted_dump = emitted_dump.or_else(|| {
            capture_stage_dump(emit, StageDumpKind::Mir, || scoop_mir::dump(mir.mir()))
        });
        let lir = mir
            .lower_lir(self.request.target().lir_target())
            .map_err(CoreBootstrapProductionError::Lir)?;
        emitted_dump = emitted_dump.or_else(|| {
            capture_stage_dump(emit, StageDumpKind::Lir, || scoop_lir::dump(lir.lir()))
        });
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
        let artifact = artifact
            .publish(self.artifact_slot.path(), limits, &core_owners)
            .map_err(CoreBootstrapProductionError::Publication)?;
        Ok(SingleConeProductionSuccess::new(
            artifact,
            warnings,
            emitted_dump,
        ))
    }
}

#[derive(Debug)]
pub enum CoreBootstrapProductionError {
    HirInput(CoreBootstrapHirInputError),
    Hir(CoreBootstrapHirStageError),
    Mir(CoreBootstrapMirStageError),
    Lir(CoreBootstrapLirStageError),
    StrongProfile(CoreBootstrapStrongProfileError),
    Warnings(super::CurrentConeDiagnosticSetError),
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
            Self::Warnings(source) => source.fmt(formatter),
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
            Self::Warnings(source) => Some(source),
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
    Resource(SlibClosureResourceErrorV1),
    Discovery(Box<SourceDiscoveryError>),
    SingleFile(Box<SingleFileInputError>),
    SourceLengthOverflow,
    Parser(Box<ParseCurrentConeError>),
}

impl CurrentConeSourceStageError {
    pub fn is_resource_limit(&self) -> bool {
        match self {
            Self::Resource(_) => true,
            Self::Discovery(source) => matches!(
                source.kind(),
                SourceDiscoveryErrorKind::FileLimitExceeded { .. }
                    | SourceDiscoveryErrorKind::ByteLimitExceeded { .. }
            ),
            Self::SingleFile(source) => matches!(
                source.kind(),
                SingleFileInputErrorKind::ByteLimitExceeded { .. }
            ),
            Self::SourceLengthOverflow | Self::Parser(_) => false,
        }
    }
}

impl fmt::Display for CurrentConeSourceStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(source) => source.fmt(formatter),
            Self::Discovery(source) => source.fmt(formatter),
            Self::SingleFile(source) => source.fmt(formatter),
            Self::SourceLengthOverflow => {
                formatter.write_str("current source length does not fit u64")
            }
            Self::Parser(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CurrentConeSourceStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(source) => Some(source),
            Self::Discovery(source) => Some(source.as_ref()),
            Self::SingleFile(source) => Some(source.as_ref()),
            Self::Parser(source) => Some(source.as_ref()),
            Self::SourceLengthOverflow => None,
        }
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
    ExplicitDependencyLoad(Box<ExplicitDependencyLoadError>),
    TrustedCoreLoad(Box<TrustedCoreArtifactLoadError>),
}

impl fmt::Display for SingleConePreflightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(source) => source.fmt(formatter),
            Self::ExplicitDependencyLoad(source) => source.fmt(formatter),
            Self::TrustedCoreLoad(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for SingleConePreflightError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Manifest(source) => Some(source.as_ref()),
            Self::ExplicitDependencyLoad(source) => Some(source.as_ref()),
            Self::TrustedCoreLoad(source) => Some(source.as_ref()),
        }
    }
}

#[cfg(test)]
mod tests;
