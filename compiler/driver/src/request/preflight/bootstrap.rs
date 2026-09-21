//! Isolated stage-fixture adapters over the shared production helpers.

use super::*;

impl TrustedCoreBootstrapHirOutput {
    #[cfg(test)]
    pub fn lower(sources: &CurrentConeParsedSources) -> Result<Self, CurrentConeHirStageError> {
        let world = scoop_hir::ImportedSemanticWorld::from_validated_closure(
            sources.cone(),
            Vec::new(),
            Vec::new(),
        )
        .map_err(CurrentConeHirStageError::SemanticWorld)?;
        Self::lower_with_world(sources, &world)
    }

    pub(super) fn lower_with_world(
        sources: &CurrentConeParsedSources,
        world: &scoop_hir::ImportedSemanticWorld<'_>,
    ) -> Result<Self, CurrentConeHirStageError> {
        let artifacts = current_hir::CurrentConeHirArtifacts::lower(
            scoop_identity::RequestedConeKind::Library,
            sources,
            scoop_hir_lower::CoreProtocolInput::CurrentDeclarations,
            world,
        )?;
        Ok(Self { artifacts })
    }
}

/// Atomic trusted-core HIR product for the single-Cone production pipeline.
///
/// The private fields prevent downstream orchestration from omitting the
/// mandatory production section. The graph already owns its output contract,
/// and both are derived during the same successful stage.
pub struct TrustedCoreBootstrapHirOutput {
    artifacts: current_hir::CurrentConeHirArtifacts,
}

impl TrustedCoreBootstrapHirOutput {
    pub const fn hir(&self) -> &scoop_hir::Output {
        self.artifacts.hir.output()
    }

    pub const fn output_kind(&self) -> &scoop_hir::ConeOutputKind {
        self.artifacts.hir.output().output_kind()
    }

    pub const fn foundation(&self) -> &scoop_hir::CanonicalHirFoundation {
        &self.artifacts.foundation
    }

    pub const fn production_section(&self) -> &scoop_hir::CoreBootstrapInterfaceSectionV1 {
        &self.artifacts.production_section
    }

    pub const fn cross_cone_section(&self) -> &scoop_hir::CrossConeHirInterfaceSectionV1 {
        &self.artifacts.cross_cone_section
    }

    /// Advances this exact sealed HIR product through MIR lowering.
    ///
    /// Consuming `self` keeps the HIR graph, its mandatory production
    /// section, the MIR graph, and the derived ODR-free foundation in one
    /// inseparable stage product.
    #[cfg(test)]
    pub fn lower_mir(self) -> Result<TrustedCoreBootstrapMirOutput, CurrentConeMirStageError> {
        let selected =
            scoop_mir::SelectedDependencyMirSet::empty(self.artifacts.hir.output().export.cone);
        let artifacts = self
            .machine_input()
            .lower_selected_mir(scoop_mir::CurrentMirProtocolDeclarations, selected)?;
        Ok(self.with_mir(artifacts))
    }

    pub(super) fn machine_input(&self) -> machine::CurrentConeMachineHir<'_> {
        self.artifacts.machine_input()
    }

    fn with_mir(
        self,
        artifacts: machine::CurrentConeMirArtifacts<scoop_mir::CurrentMirProtocolDeclarations>,
    ) -> TrustedCoreBootstrapMirOutput {
        TrustedCoreBootstrapMirOutput {
            hir: self,
            strong: artifacts.strong,
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
    cross_cone_bridge: scoop_mir::CrossConeMirBridgeSectionV1,
}

impl TrustedCoreBootstrapMirOutput {
    pub const fn hir(&self) -> &TrustedCoreBootstrapHirOutput {
        &self.hir
    }

    pub const fn mir(&self) -> &scoop_mir::Module {
        self.strong.module()
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
    pub const fn lir(&self) -> &scoop_lir::Module {
        self.lir.module()
    }

    pub const fn strong_lir_output(&self) -> &scoop_lir::SingleConeStrongLirOutput {
        &self.lir
    }

    pub const fn foundation(&self) -> &scoop_lir::OdrFreeLirFoundation {
        self.lir.foundation()
    }

    pub const fn shape_support(&self) -> &scoop_lir::StrongLirShapeSupportPlan {
        self.lir.shape_support()
    }

    /// Seals all three IR foundations under the strong profile's `RejectAll`
    /// policy before object production can observe this lowering result.
    pub fn seal_strong_profile(
        self,
    ) -> Result<CrossConeStrongIrProductionV1, CurrentConeStrongProfileError> {
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
        hir.artifacts
            .seal_strong_profile(strong, mir_cross_cone, lir, lir_cross_cone)
    }
}
