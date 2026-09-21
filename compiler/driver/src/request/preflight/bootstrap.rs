//! Core library lowering and publication.

use super::*;

mod errors;
pub use errors::*;

pub struct ParsedCoreBootstrapBuildRequest<'request, 'artifact> {
    pub(super) request: &'request ValidatedCoreOnlyBuildRequest<'artifact>,
    pub(super) sources: CurrentConeParsedSources,
}

impl<'request, 'artifact> ParsedCoreBootstrapBuildRequest<'request, 'artifact> {
    pub const fn request(&self) -> &'request ValidatedCoreOnlyBuildRequest<'artifact> {
        self.request
    }

    pub const fn sources(&self) -> &CurrentConeParsedSources {
        &self.sources
    }

    /// Builds the core library and publishes to the requested output path.
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
        let world = self
            .request
            .dependencies()
            .semantic()
            .imported_semantic_world()
            .map_err(|error| {
                CoreBootstrapProductionError::Hir(CoreBootstrapHirStageError::SemanticWorld(error))
            })?;
        let hir = TrustedCoreBootstrapHirOutput::lower_with_world(&self.sources, &world)
            .map_err(CoreBootstrapProductionError::Hir)?;
        let warnings = CurrentConeDiagnosticSet::try_new(hir.hir().warnings.clone(), &self.sources)
            .map_err(CoreBootstrapProductionError::Warnings)?;
        emitted_dump = emitted_dump.or_else(|| {
            capture_stage_dump(emit, StageDumpKind::Hir, || {
                scoop_hir::dump(&hir.hir().export)
            })
        });
        let mir = hir
            .lower_mir_with_dependencies(self.request.dependencies().semantic())
            .map_err(CoreBootstrapProductionError::Mir)?;
        emitted_dump = emitted_dump.or_else(|| {
            capture_stage_dump(emit, StageDumpKind::Mir, || scoop_mir::dump(mir.mir()))
        });
        let lir = mir
            .lower_lir_with_dependencies(
                self.request.dependencies().semantic(),
                self.request.target().lir_target(),
            )
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
                self.request.dependencies().direct_dependencies().to_vec(),
                temporary_parent,
                self.request.target(),
                &core_owners,
            )
            .map_err(CoreBootstrapProductionError::Artifact)?;
        let artifact = artifact
            .publish(
                self.request.output().as_path(),
                self.request.dependencies().dependency_first().to_vec(),
                limits,
            )
            .map_err(CoreBootstrapProductionError::Publication)?;
        Ok(SingleConeProductionSuccess::new_cross_cone(
            artifact,
            warnings,
            emitted_dump,
        ))
    }
}

impl TrustedCoreBootstrapHirOutput {
    #[cfg(test)]
    pub fn lower(sources: &CurrentConeParsedSources) -> Result<Self, CoreBootstrapHirStageError> {
        let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
            sources.cone(),
            Vec::new(),
            Vec::new(),
        )
        .map_err(CoreBootstrapHirStageError::SemanticWorld)?;
        Self::lower_with_world(sources, &world)
    }

    pub(super) fn lower_with_world(
        sources: &CurrentConeParsedSources,
        world: &scoop_hir::ImportedSemanticWorld<'_>,
    ) -> Result<Self, CoreBootstrapHirStageError> {
        let sources = scoop_hir_lower::CurrentConeSources::try_new(
            sources,
            scoop_hir_lower::CoreProtocolInput::CurrentDeclarations,
            world,
        )
        .map_err(CoreBootstrapHirStageError::Sources)?;
        let hir = scoop_hir_lower::lower_current_cone(
            scoop_identity::RequestedConeKind::Library,
            &sources,
        )
        .map_err(CoreBootstrapHirStageError::Lowering)?;
        let mut foundation = scoop_hir::CanonicalHirFoundation::from_dependency_output(&hir)
            .map_err(CoreBootstrapHirStageError::Foundation)?;
        let production_section =
            scoop_hir::CoreBootstrapInterfaceSectionV1::from_export(&hir.output().export)
                .map_err(CoreBootstrapHirStageError::ProductionSection)?;
        let cross_cone_section = {
            let mut authority = scoop_hir::CrossConeHirProductionAuthority::new(
                &foundation,
                &hir.output().export.public_export_bindings,
                world,
            );
            scoop_hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(
                &hir,
                &[],
                &mut authority,
            )
            .map_err(|source| CoreBootstrapHirStageError::CrossConeSection(Box::new(source)))?
        };
        let core_classifier =
            scoop_hir::CoreClosedExactLeafClassifierV1::try_from_nominal_interfaces(
                cross_cone_section.nominal_interfaces().records(),
            )
            .map_err(CoreBootstrapHirStageError::CoreClassifier)?;
        foundation
            .complete_cross_cone_source_points(
                hir.output().export.module(),
                cross_cone_section.definition_sources(),
            )
            .map_err(CoreBootstrapHirStageError::Foundation)?;
        Ok(TrustedCoreBootstrapHirOutput {
            hir,
            foundation,
            production_section,
            cross_cone_section,
            core_classifier,
        })
    }
}

/// Atomic trusted-core HIR product for the single-Cone production pipeline.
///
/// The private fields prevent downstream orchestration from omitting the
/// mandatory production section. The graph already owns its output contract,
/// and both are derived during the same successful stage.
pub struct TrustedCoreBootstrapHirOutput {
    hir: scoop_hir::DependencyHirOutput,
    foundation: scoop_hir::CanonicalHirFoundation,
    production_section: scoop_hir::CoreBootstrapInterfaceSectionV1,
    cross_cone_section: scoop_hir::CrossConeHirInterfaceSectionV1,
    core_classifier: scoop_hir::CoreClosedExactLeafClassifierV1,
}

impl TrustedCoreBootstrapHirOutput {
    pub const fn hir(&self) -> &scoop_hir::Output {
        self.hir.output()
    }

    pub const fn output_kind(&self) -> &scoop_hir::ConeOutputKind {
        self.hir.output().output_kind()
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

    /// Advances this exact sealed HIR product through MIR lowering.
    ///
    /// Consuming `self` keeps the HIR graph, its mandatory production
    /// section, the MIR graph, and the derived ODR-free foundation in one
    /// inseparable stage product.
    #[cfg(test)]
    pub fn lower_mir(self) -> Result<TrustedCoreBootstrapMirOutput, CurrentConeMirStageError> {
        let selected = scoop_mir::SelectedDependencyMirSet::empty(self.hir.output().export.cone);
        let artifacts = self
            .machine_input()
            .lower_selected_mir(scoop_mir::CurrentMirProtocolDeclarations, selected)?;
        Ok(self.with_mir(artifacts))
    }

    fn lower_mir_with_dependencies(
        self,
        closure: &scoop_slib::ValidatedCrossConeSemanticClosure<'_>,
    ) -> Result<TrustedCoreBootstrapMirOutput, CurrentConeMirStageError> {
        let artifacts = self
            .machine_input()
            .lower_mir(scoop_mir::CurrentMirProtocolDeclarations, closure)?;
        Ok(self.with_mir(artifacts))
    }

    pub(super) fn machine_input(&self) -> machine::CurrentConeMachineHir<'_> {
        machine::CurrentConeMachineHir {
            output: &self.hir,
            foundation: &self.foundation,
            production: &self.production_section,
            public: &self.cross_cone_section,
            classifier: &self.core_classifier,
        }
    }

    fn with_mir(
        self,
        artifacts: machine::CurrentConeMirArtifacts<scoop_mir::CurrentMirProtocolDeclarations>,
    ) -> TrustedCoreBootstrapMirOutput {
        TrustedCoreBootstrapMirOutput {
            hir: self,
            strong: artifacts.strong,
            selected_dependencies: artifacts.dependencies,
            cross_cone_bridge: artifacts.public,
        }
    }
}

/// Atomic trusted-core MIR product for the single-Cone strong pipeline.
///
/// The previous HIR stage is owned rather than referenced so no caller can
/// pair this MIR graph or production section with a different HIR proof.
pub struct TrustedCoreBootstrapMirOutput {
    hir: TrustedCoreBootstrapHirOutput,
    strong: scoop_mir::SingleConeStrongMirInput,
    selected_dependencies: scoop_mir::SelectedDependencyMirSet,
    cross_cone_bridge: scoop_mir::CrossConeMirBridgeSectionV1,
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

    pub const fn cross_cone_bridge(&self) -> &scoop_mir::CrossConeMirBridgeSectionV1 {
        &self.cross_cone_bridge
    }

    pub const fn materialization_plan(&self) -> &scoop_mir::SingleConeStrongMaterializationPlan {
        self.strong.materialization()
    }

    /// Advances this exact sealed MIR product through strong LIR lowering.
    ///
    /// Consuming `self` keeps the complete bootstrap proof chain attached to
    /// the resulting ODR-free LIR graph.
    #[cfg(test)]
    pub fn lower_lir(
        self,
        target_profile: scoop_lir::LirTargetProfile,
    ) -> Result<TrustedCoreBootstrapLirOutput, CurrentConeLirStageError> {
        let selected = scoop_lir::SelectedDependencyLirSet::try_from_callables(
            self.strong.module().cone,
            Vec::new(),
        )
        .expect("empty test selection");
        let (lir, cross_cone_bridge) = machine::lower_selected_lir(
            &self.strong,
            &self.cross_cone_bridge,
            scoop_lir_lower::StrongImportedCoreLirInput::Unused,
            &selected,
            target_profile,
        )?;
        Ok(TrustedCoreBootstrapLirOutput {
            mir: self,
            lir,
            cross_cone_bridge,
        })
    }

    fn lower_lir_with_dependencies(
        self,
        closure: &scoop_slib::ValidatedCrossConeSemanticClosure<'_>,
        target_profile: scoop_lir::LirTargetProfile,
    ) -> Result<TrustedCoreBootstrapLirOutput, CurrentConeLirStageError> {
        let (lir, cross_cone_bridge) = machine::lower_lir(
            &self.strong,
            &self.cross_cone_bridge,
            scoop_lir_lower::StrongImportedCoreLirInput::Unused,
            &self.selected_dependencies,
            closure,
            target_profile,
        )?;
        Ok(TrustedCoreBootstrapLirOutput {
            mir: self,
            lir,
            cross_cone_bridge,
        })
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
    cross_cone_bridge: scoop_lir::CrossConeLirBridgeSectionV1,
}

impl TrustedCoreBootstrapLirOutput {
    pub const fn mir_stage(&self) -> &TrustedCoreBootstrapMirOutput {
        &self.mir
    }

    pub const fn lir(&self) -> &scoop_lir::Module {
        self.lir.module()
    }

    pub const fn strong_lir_output(&self) -> &scoop_lir::SingleConeStrongLirOutput {
        &self.lir
    }

    pub const fn foundation(&self) -> &scoop_lir::OdrFreeLirFoundation {
        self.lir.foundation()
    }

    pub const fn core_shape_support(&self) -> &scoop_lir::StrongLirCoreShapeSupportPlan {
        self.lir.core_shape_support()
    }

    pub const fn cross_cone_bridge(&self) -> &scoop_lir::CrossConeLirBridgeSectionV1 {
        &self.cross_cone_bridge
    }

    /// Seals all three IR foundations under the strong profile's `RejectAll`
    /// policy before object production can observe this lowering result.
    pub fn seal_strong_profile(
        self,
    ) -> Result<CrossConeStrongIrProductionV1, CoreBootstrapStrongProfileError> {
        let hir_foundation =
            scoop_hir::OdrFreeHirFoundation::try_new(self.mir.hir.foundation.clone())
                .map_err(CoreBootstrapStrongProfileError::HirOdr)?;
        let Self {
            mir,
            lir,
            cross_cone_bridge: lir_cross_cone,
        } = self;
        let TrustedCoreBootstrapMirOutput {
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
