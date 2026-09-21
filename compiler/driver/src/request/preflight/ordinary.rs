use super::*;

mod errors;
pub use errors::*;

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
        let world = self
            .request
            .dependencies()
            .semantic()
            .imported_semantic_world()
            .map_err(OrdinaryConeHirStageError::SemanticWorld)?;
        let core = self
            .trusted_core
            .import_core_inputs()
            .map_err(OrdinaryConeHirStageError::CoreInterface)?;
        let nominals = world
            .direct_provider(scoop_identity::ConeIdentity::CORE)
            .map(|provider| provider.nominal_interfaces().records())
            .unwrap_or_default();
        let core_classifier =
            scoop_hir::CoreClosedExactLeafClassifierV1::try_from_nominal_interfaces(nominals)
                .map_err(OrdinaryConeHirStageError::CoreClassifier)?;
        let input = scoop_hir_lower::CurrentConeSources::try_new(&self.sources, core, &world)
            .map_err(OrdinaryConeHirStageError::Input)?;
        let hir = scoop_hir_lower::lower_current_cone(requested, &input)
            .map_err(OrdinaryConeHirStageError::Lowering)?;
        let mut foundation = scoop_hir::CanonicalHirFoundation::from_dependency_output(&hir)
            .map_err(OrdinaryConeHirStageError::Foundation)?;
        let production_section =
            scoop_hir::CoreBootstrapInterfaceSectionV1::from_export(&hir.output().export)
                .map_err(OrdinaryConeHirStageError::ProductionSection)?;
        let cross_cone_section = {
            let mut authority = scoop_hir::CrossConeHirProductionAuthority::new(
                &foundation,
                &hir.output().export.public_export_bindings,
                &world,
            );
            scoop_hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(
                &hir,
                &[],
                &mut authority,
            )
            .map_err(OrdinaryConeHirStageError::CrossConeSection)?
        };
        foundation
            .complete_cross_cone_source_points(
                hir.output().export.module(),
                cross_cone_section.definition_sources(),
            )
            .map_err(OrdinaryConeHirStageError::Foundation)?;
        Ok(OrdinaryConeHirOutput {
            trusted_core: self.trusted_core,
            dependencies: self.request.dependencies(),
            target_profile: self.request.target().lir_target(),
            hir,
            foundation,
            production_section,
            cross_cone_section,
            core_classifier,
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
        let direct_dependencies = self.request.dependencies().direct_dependencies().to_vec();
        let dependency_first = self.request.dependencies().dependency_first().to_vec();
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
            .publish(self.request.output().as_path(), dependency_first, limits)
            .map_err(OrdinaryConeProductionError::Publication)?;
        Ok(SingleConeProductionSuccess::new_cross_cone(
            artifact,
            warnings,
            emitted_dump,
        ))
    }
}

/// Owned ordinary HIR with the validated dependency closure borrowed for
/// subsequent machine IR projections.
pub struct OrdinaryConeHirOutput<'stage, 'artifact> {
    trusted_core: &'stage ValidatedTrustedCoreArtifact<'artifact>,
    dependencies: &'stage ValidatedExplicitDependencyInputSet<'artifact>,
    target_profile: scoop_lir::LirTargetProfile,
    hir: scoop_hir::DependencyHirOutput,
    foundation: scoop_hir::CanonicalHirFoundation,
    production_section: scoop_hir::CoreBootstrapInterfaceSectionV1,
    cross_cone_section: scoop_hir::CrossConeHirInterfaceSectionV1,
    core_classifier: scoop_hir::CoreClosedExactLeafClassifierV1,
}

impl<'stage, 'artifact> OrdinaryConeHirOutput<'stage, 'artifact> {
    pub const fn hir(&self) -> &scoop_hir::DependencyHirOutput {
        &self.hir
    }

    pub const fn foundation(&self) -> &scoop_hir::CanonicalHirFoundation {
        &self.foundation
    }

    pub const fn production_section(&self) -> &scoop_hir::CoreBootstrapInterfaceSectionV1 {
        &self.production_section
    }

    pub const fn cross_cone_section(&self) -> &scoop_hir::CrossConeHirInterfaceSectionV1 {
        &self.cross_cone_section
    }

    /// Projects the complete HIR selection through the same trusted artifact,
    /// lowers direct external calls, and immediately seals the owned MIR
    /// materialization roots.
    pub fn lower_mir(
        self,
    ) -> Result<OrdinaryConeMirOutput<'stage, 'artifact>, OrdinaryConeMirStageError> {
        let selected = self
            .trusted_core
            .project_initialization_protocol_to_mir(
                !self
                    .hir
                    .output()
                    .local
                    .module()
                    .initialization_units
                    .is_empty(),
            )
            .map_err(OrdinaryConeMirStageError::Projection)?;
        let dependency_selection = self
            .dependencies
            .semantic()
            .project_dependency_callables_to_mir(self.hir.imported_dependencies())
            .map_err(OrdinaryConeMirStageError::DependencyProjection)?;
        let mir = scoop_mir_lower::lower_current_cone(&self.hir, selected, dependency_selection)
            .map_err(OrdinaryConeMirStageError::Lowering)?;
        let (module, selected, selected_dependencies) = mir.into_parts();
        let foundation = scoop_mir::OdrFreeMirFoundation::from_module(&module)
            .map_err(OrdinaryConeMirStageError::Foundation)?;
        let production_section = scoop_mir_lower::lower_production_section(
            module.cone,
            &self.production_section,
            &self.foundation,
            &foundation,
        )
        .map_err(OrdinaryConeMirStageError::ProductionSection)?;
        let cross_cone_bridge = scoop_mir_lower::lower_cross_cone_bridge_section(
            module.cone,
            &self.cross_cone_section,
            &self.core_classifier,
            &foundation,
            &selected_dependencies,
        )
        .map_err(OrdinaryConeMirStageError::CrossConeBridge)?;
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
            selected_dependencies,
            cross_cone_bridge,
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
    selected_dependencies: scoop_mir::SelectedDependencyMirSet,
    cross_cone_bridge: scoop_mir::CrossConeMirBridgeSectionV1,
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

    pub const fn cross_cone_bridge(&self) -> &scoop_mir::CrossConeMirBridgeSectionV1 {
        &self.cross_cone_bridge
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
        let selected_dependencies = self
            .hir
            .dependencies
            .semantic()
            .project_dependency_callables_to_lir(&self.selected_dependencies)
            .map_err(OrdinaryConeLirStageError::DependencyProjection)?;
        let lir = scoop_lir_lower::lower_with_dependencies(
            &self.strong,
            scoop_lir_lower::StrongImportedCoreLirInput::Selected(&selected),
            scoop_lir_lower::StrongImportedDependencyLirInput::Selected(&selected_dependencies),
            self.hir.target_profile,
        )
        .map_err(OrdinaryConeLirStageError::Lowering)?;
        let cross_cone_bridge = scoop_lir_lower::lower_cross_cone_bridge_section(
            &self.strong,
            &self.cross_cone_bridge,
            &lir,
        )
        .map_err(OrdinaryConeLirStageError::CrossConeBridge)?;
        Ok(OrdinaryConeLirOutput {
            mir: self,
            lir,
            cross_cone_bridge,
        })
    }
}

/// Atomic ordinary LIR product whose entire imported authority has already
/// been converted into owned external callable definitions and requirements.
pub struct OrdinaryConeLirOutput<'stage, 'artifact> {
    mir: OrdinaryConeMirOutput<'stage, 'artifact>,
    lir: scoop_lir::SingleConeStrongLirOutput,
    cross_cone_bridge: scoop_lir::CrossConeLirBridgeSectionV1,
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
    ) -> Result<CrossConeStrongIrProductionV1, OrdinaryConeStrongProfileError> {
        let hir_foundation =
            scoop_hir::OdrFreeHirFoundation::try_new(self.mir.hir.foundation.clone())
                .map_err(OrdinaryConeStrongProfileError::HirOdr)?;
        let Self {
            mir,
            lir,
            cross_cone_bridge: lir_cross_cone,
        } = self;
        let OrdinaryConeMirOutput {
            hir,
            strong,
            cross_cone_bridge: mir_cross_cone,
            ..
        } = mir;
        Ok(CrossConeStrongIrProductionV1::new(
            hir_foundation,
            hir.production_section,
            hir.cross_cone_section,
            strong.foundation().clone(),
            strong.production().clone(),
            mir_cross_cone,
            lir,
            lir_cross_cone,
        ))
    }
}
