use super::*;

impl<'request, 'artifact> ParsedOrdinaryConeBuildRequest<'request, 'artifact> {
    /// Lowers this request through the ordinary HIR boundary while retaining
    /// the exact trusted-core artifact that owns every selected import.
    pub fn lower_hir<'stage>(
        &'stage self,
    ) -> Result<OrdinaryConeHirOutput<'stage, 'artifact>, OrdinaryConeHirStageError> {
        let requested = match self.request.current() {
            ValidatedCurrentConeInput::Manifest { manifest, .. } => {
                manifest.parsed().semantic().requested_kind()
            }
            ValidatedCurrentConeInput::SingleFile { .. } => {
                scoop_identity::RequestedConeKind::Executable
            }
            ValidatedCurrentConeInput::TrustedCoreBootstrap { .. } => {
                panic!("an ordinary parsed request cannot contain trusted-core bootstrap input")
            }
        };
        let input = self.hir_input().map_err(OrdinaryConeHirStageError::Input)?;
        let hir = scoop_hir_lower::lower_ordinary_core_only(requested, &input)
            .map_err(OrdinaryConeHirStageError::Lowering)?;
        let foundation = scoop_hir::CanonicalHirFoundation::from_ordinary_output(&hir)
            .map_err(OrdinaryConeHirStageError::Foundation)?;
        let production_section =
            scoop_hir::CoreBootstrapInterfaceSectionV1::from_export(&hir.output().export)
                .map_err(OrdinaryConeHirStageError::ProductionSection)?;
        Ok(OrdinaryConeHirOutput {
            trusted_core: self.trusted_core,
            target_profile: self.request.target().lir_target(),
            hir,
            foundation,
            production_section,
        })
    }

    /// Runs the only ordinary single-Cone production path and atomically
    /// publishes its independently revalidated `.slib` artifact.
    pub fn build_and_publish(
        self,
        temporary_parent: &Path,
        limits: DecodeLimits,
    ) -> Result<SingleConeProductionSuccess, OrdinaryConeProductionError> {
        let cone = match self.request.current() {
            ValidatedCurrentConeInput::Manifest { manifest, .. } => {
                let semantic = manifest.parsed().semantic();
                let kind = match semantic.requested_kind() {
                    scoop_identity::RequestedConeKind::Library => scoop_slib::ConeKind::Library,
                    scoop_identity::RequestedConeKind::Executable => {
                        scoop_slib::ConeKind::Executable
                    }
                };
                scoop_slib::ConeRecord::new(
                    semantic.coordinate().clone(),
                    kind,
                    scoop_slib::ConeSourceForm::Manifest,
                )
                .map_err(OrdinaryConeProductionError::Cone)?
            }
            ValidatedCurrentConeInput::SingleFile { .. } => scoop_slib::ConeRecord::new(
                ConeCoordinate::reserved_single_file(),
                scoop_slib::ConeKind::Executable,
                scoop_slib::ConeSourceForm::SingleFile,
            )
            .map_err(OrdinaryConeProductionError::Cone)?,
            ValidatedCurrentConeInput::TrustedCoreBootstrap { .. } => {
                panic!("an ordinary parsed request cannot contain trusted-core bootstrap input")
            }
        };
        let producer =
            scoop_slib::ProducerRecord::new(concat!("scoopc/", env!("CARGO_PKG_VERSION")))
                .map_err(OrdinaryConeProductionError::Producer)?;
        let direct_dependencies = vec![self.trusted_core.dependency_record()];
        let emit = self.request.emit();
        let mut emitted_dump = capture_stage_dump(emit, StageDumpKind::Ast, || {
            self.sources
                .sources()
                .sources()
                .iter()
                .map(|source| scoop_ast::dump(source.ast()))
                .collect()
        });
        let hir = self.lower_hir().map_err(OrdinaryConeProductionError::Hir)?;
        let warnings =
            CurrentConeDiagnosticSet::try_new(hir.hir().output().warnings.clone(), &self.sources)
                .map_err(OrdinaryConeProductionError::Warnings)?;
        emitted_dump = emitted_dump.or_else(|| {
            capture_stage_dump(emit, StageDumpKind::Hir, || {
                scoop_hir::dump(&hir.hir().output().export)
            })
        });
        let mir = hir.lower_mir().map_err(OrdinaryConeProductionError::Mir)?;
        emitted_dump = emitted_dump.or_else(|| {
            capture_stage_dump(emit, StageDumpKind::Mir, || scoop_mir::dump(mir.mir()))
        });
        let lir = mir.lower_lir().map_err(OrdinaryConeProductionError::Lir)?;
        emitted_dump = emitted_dump.or_else(|| {
            capture_stage_dump(emit, StageDumpKind::Lir, || scoop_lir::dump(lir.lir()))
        });
        let artifact = lir
            .seal_strong_profile()
            .map_err(OrdinaryConeProductionError::StrongProfile)?
            .produce_artifact(
                producer,
                cone,
                direct_dependencies,
                temporary_parent,
                self.request.target(),
                self.trusted_core.defined_symbols(),
            )
            .map_err(OrdinaryConeProductionError::Artifact)?;
        let artifact = artifact
            .publish(
                self.request.output().as_path(),
                limits,
                self.trusted_core.defined_symbols(),
            )
            .map_err(OrdinaryConeProductionError::Publication)?;
        Ok(SingleConeProductionSuccess::new(
            artifact,
            warnings,
            emitted_dump,
        ))
    }
}

#[derive(Debug)]
pub enum OrdinaryConeProductionError {
    Hir(OrdinaryConeHirStageError),
    Mir(OrdinaryConeMirStageError),
    Lir(OrdinaryConeLirStageError),
    StrongProfile(OrdinaryConeStrongProfileError),
    Warnings(super::CurrentConeDiagnosticSetError),
    Producer(scoop_slib::ProducerRecordError),
    Cone(scoop_slib::ConeRecordError),
    Artifact(crate::StrongIrArtifactProductionError),
    Publication(crate::StrongArtifactProductionError),
}

impl fmt::Display for OrdinaryConeProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

impl std::error::Error for OrdinaryConeProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Hir(source) => source,
            Self::Mir(source) => source,
            Self::Lir(source) => source,
            Self::StrongProfile(source) => source,
            Self::Warnings(source) => source,
            Self::Producer(source) => source,
            Self::Cone(source) => source,
            Self::Artifact(source) => source,
            Self::Publication(source) => source,
        })
    }
}

/// Atomic ordinary HIR product whose imported uses remain borrowed from the
/// exact trusted artifact selected during request validation.
pub struct OrdinaryConeHirOutput<'stage, 'artifact> {
    trusted_core: &'stage ValidatedTrustedCoreArtifact<'artifact>,
    target_profile: scoop_lir::LirTargetProfile,
    hir: scoop_hir::OrdinaryHirOutput<'stage>,
    foundation: scoop_hir::CanonicalHirFoundation,
    production_section: scoop_hir::CoreBootstrapInterfaceSectionV1,
}

impl<'stage, 'artifact> OrdinaryConeHirOutput<'stage, 'artifact> {
    pub const fn hir(&self) -> &scoop_hir::OrdinaryHirOutput<'stage> {
        &self.hir
    }

    pub const fn foundation(&self) -> &scoop_hir::CanonicalHirFoundation {
        &self.foundation
    }

    pub const fn production_section(&self) -> &scoop_hir::CoreBootstrapInterfaceSectionV1 {
        &self.production_section
    }

    /// Projects the complete HIR selection through the same trusted artifact,
    /// lowers direct external calls, and immediately seals the owned MIR
    /// materialization roots.
    pub fn lower_mir(
        self,
    ) -> Result<OrdinaryConeMirOutput<'stage, 'artifact>, OrdinaryConeMirStageError> {
        let selected = self
            .trusted_core
            .project_core_callables_to_mir(
                self.hir.imported_core(),
                !self
                    .hir
                    .output()
                    .local
                    .module()
                    .initialization_units
                    .is_empty(),
            )
            .map_err(OrdinaryConeMirStageError::Projection)?;
        let dependency_selection =
            scoop_mir::SelectedDependencyMirSet::empty(self.hir.output().local.module().cone);
        let mir = scoop_mir_lower::lower_ordinary(&self.hir, selected, dependency_selection)
            .map_err(OrdinaryConeMirStageError::Lowering)?;
        let (module, selected, selected_dependencies) = mir.into_parts();
        let foundation = scoop_mir::OdrFreeMirFoundation::from_module(&module)
            .map_err(OrdinaryConeMirStageError::Foundation)?;
        let production_section = scoop_mir_lower::lower_production_section(
            module.cone,
            &self.production_section,
            &foundation,
        )
        .map_err(OrdinaryConeMirStageError::ProductionSection)?;
        let strong = scoop_mir::SingleConeStrongMirInput::try_new_with_dependencies(
            module,
            foundation,
            production_section,
            scoop_mir::CoreShapeSupportSourceInput::NotCore,
            scoop_mir::StrongImportedCoreInput::Selected(&selected),
            scoop_mir::StrongImportedDependencyInput::Selected(&selected_dependencies),
        )
        .map_err(OrdinaryConeMirStageError::Sealing)?;
        Ok(OrdinaryConeMirOutput {
            hir: self,
            strong,
            selected,
        })
    }
}

/// Atomic ordinary MIR product. The strong MIR graph owns persistent imported
/// roots, while the retained selected set remains available only for the
/// matching trusted-artifact LIR projection.
pub struct OrdinaryConeMirOutput<'stage, 'artifact> {
    hir: OrdinaryConeHirOutput<'stage, 'artifact>,
    strong: scoop_mir::SingleConeStrongMirInput,
    selected: scoop_mir::SelectedImportedMirSet<'stage>,
}

impl<'stage, 'artifact> OrdinaryConeMirOutput<'stage, 'artifact> {
    pub const fn hir(&self) -> &OrdinaryConeHirOutput<'_, '_> {
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

    /// Projects the MIR selection into the matching trusted-core LIR
    /// authority and consumes it immediately during strong LIR lowering.
    pub fn lower_lir(
        self,
    ) -> Result<OrdinaryConeLirOutput<'stage, 'artifact>, OrdinaryConeLirStageError> {
        let selected = self
            .hir
            .trusted_core
            .project_core_callables_to_lir(&self.selected)
            .map_err(OrdinaryConeLirStageError::Projection)?;
        let lir = scoop_lir_lower::lower(
            &self.strong,
            scoop_lir_lower::StrongImportedCoreLirInput::Selected(&selected),
            self.hir.target_profile,
        )
        .map_err(OrdinaryConeLirStageError::Lowering)?;
        Ok(OrdinaryConeLirOutput { mir: self, lir })
    }
}

/// Atomic ordinary LIR product whose entire imported authority has already
/// been converted into owned external callable definitions and requirements.
pub struct OrdinaryConeLirOutput<'stage, 'artifact> {
    mir: OrdinaryConeMirOutput<'stage, 'artifact>,
    lir: scoop_lir::SingleConeStrongLirOutput,
}

impl OrdinaryConeLirOutput<'_, '_> {
    pub const fn mir_stage(&self) -> &OrdinaryConeMirOutput<'_, '_> {
        &self.mir
    }

    pub const fn lir(&self) -> &scoop_lir::Module {
        self.lir.module()
    }

    pub const fn foundation(&self) -> &scoop_lir::OdrFreeLirFoundation {
        self.lir.foundation()
    }

    /// Seals the three stage foundations as one strong production input.
    pub fn seal_strong_profile(
        self,
    ) -> Result<SingleConeStrongIrProductionV1, OrdinaryConeStrongProfileError> {
        let hir_foundation =
            scoop_hir::OdrFreeHirFoundation::try_new(self.mir.hir.foundation.clone())
                .map_err(OrdinaryConeStrongProfileError::HirOdr)?;
        let Self { mir, lir } = self;
        let OrdinaryConeMirOutput { hir, strong, .. } = mir;
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
pub enum OrdinaryConeStrongProfileError {
    HirOdr(scoop_hir::OdrFreeHirFoundationError),
}

impl fmt::Display for OrdinaryConeStrongProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HirOdr(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for OrdinaryConeStrongProfileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HirOdr(source) => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum OrdinaryConeLirStageError {
    Projection(crate::TrustedCoreLirSetProjectionError),
    Lowering(scoop_lir_lower::StrongLirLoweringError),
}

impl fmt::Display for OrdinaryConeLirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projection(source) => source.fmt(formatter),
            Self::Lowering(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for OrdinaryConeLirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Projection(source) => source,
            Self::Lowering(source) => source,
        })
    }
}

#[derive(Debug)]
pub enum OrdinaryConeMirStageError {
    Projection(crate::TrustedCoreCallableSetProjectionError),
    Lowering(scoop_mir_lower::ImportedCoreMirLoweringError),
    Foundation(scoop_mir::OdrFreeMirFoundationProjectionError),
    ProductionSection(scoop_mir_lower::MirProductionLoweringError),
    Sealing(scoop_mir::SingleConeStrongMirInputError),
}

impl fmt::Display for OrdinaryConeMirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projection(source) => source.fmt(formatter),
            Self::Lowering(source) => source.fmt(formatter),
            Self::Foundation(source) => source.fmt(formatter),
            Self::ProductionSection(source) => source.fmt(formatter),
            Self::Sealing(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for OrdinaryConeMirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Projection(source) => source,
            Self::Lowering(source) => source,
            Self::Foundation(source) => source,
            Self::ProductionSection(source) => source,
            Self::Sealing(source) => source,
        })
    }
}

#[derive(Debug)]
pub enum OrdinaryConeHirStageError {
    Input(OrdinaryCoreOnlyHirInputError),
    Lowering(Vec<scoop_ast::Diagnostic>),
    Foundation(scoop_hir::HirFoundationBuildError),
    ProductionSection(scoop_hir::CoreBootstrapInterfaceBuildError),
}

impl fmt::Display for OrdinaryConeHirStageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(source) => source.fmt(formatter),
            Self::Lowering(diagnostics) => write!(
                formatter,
                "ordinary HIR lowering failed with {} diagnostic(s)",
                diagnostics.len()
            ),
            Self::Foundation(source) => source.fmt(formatter),
            Self::ProductionSection(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for OrdinaryConeHirStageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Input(source) => Some(source),
            Self::Lowering(_) => None,
            Self::Foundation(source) => Some(source),
            Self::ProductionSection(source) => Some(source),
        }
    }
}
