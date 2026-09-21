//! One complete HIR product and shared strong-profile sealing for every Cone.

mod errors;
pub use errors::{CurrentConeHirStageError, CurrentConeStrongProfileError};

pub(super) struct CurrentConeHirArtifacts {
    pub hir: scoop_hir::DependencyHirOutput,
    pub foundation: scoop_hir::CanonicalHirFoundation,
    pub production_section: scoop_hir::CoreBootstrapInterfaceSectionV1,
    pub cross_cone_section: scoop_hir::CrossConeHirInterfaceSectionV1,
    pub core_classifier: scoop_hir::CoreClosedExactLeafClassifierV1,
}

impl CurrentConeHirArtifacts {
    pub fn lower(
        requested: scoop_identity::RequestedConeKind,
        sources: &scoop_ast::CurrentConeParsedSources,
        protocols: scoop_hir_lower::CoreProtocolInput,
        world: &scoop_hir::ImportedSemanticWorld<'_>,
    ) -> Result<Self, CurrentConeHirStageError> {
        let sources = scoop_hir_lower::CurrentConeSources::try_new(sources, protocols, world)
            .map_err(CurrentConeHirStageError::Input)?;
        let hir = scoop_hir_lower::lower_current_cone(requested, &sources)
            .map_err(CurrentConeHirStageError::Lowering)?;
        let mut foundation = scoop_hir::CanonicalHirFoundation::from_dependency_output(&hir)
            .map_err(CurrentConeHirStageError::Foundation)?;
        let production_section =
            scoop_hir::CoreBootstrapInterfaceSectionV1::from_export(&hir.output().export)
                .map_err(CurrentConeHirStageError::ProductionSection)?;
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
            .map_err(|error| CurrentConeHirStageError::CrossConeSection(Box::new(error)))?
        };
        let nominals = match &hir.output().local.module().core_protocols {
            scoop_hir::concrete::ConcreteCoreProtocols::Defined(_) => {
                cross_cone_section.nominal_interfaces().records()
            }
            scoop_hir::concrete::ConcreteCoreProtocols::Imported(_) => world
                .direct_provider(scoop_identity::ConeIdentity::CORE)
                .map(|provider| provider.nominal_interfaces().records())
                .unwrap_or_default(),
        };
        let core_classifier =
            scoop_hir::CoreClosedExactLeafClassifierV1::try_from_nominal_interfaces(nominals)
                .map_err(CurrentConeHirStageError::CoreClassifier)?;
        foundation
            .complete_cross_cone_source_points(
                hir.output().export.module(),
                cross_cone_section.definition_sources(),
            )
            .map_err(CurrentConeHirStageError::Foundation)?;
        Ok(Self {
            hir,
            foundation,
            production_section,
            cross_cone_section,
            core_classifier,
        })
    }

    pub fn machine_input(&self) -> super::machine::CurrentConeMachineHir<'_> {
        super::machine::CurrentConeMachineHir {
            output: &self.hir,
            foundation: &self.foundation,
            production: &self.production_section,
            public: &self.cross_cone_section,
            classifier: &self.core_classifier,
        }
    }

    pub fn seal_strong_profile(
        self,
        strong: scoop_mir::SingleConeStrongMirInput,
        mir_cross_cone: scoop_mir::CrossConeMirBridgeSectionV1,
        lir: scoop_lir::SingleConeStrongLirOutput,
        lir_cross_cone: scoop_lir::CrossConeLirBridgeSectionV1,
    ) -> Result<crate::CrossConeStrongIrProductionV1, CurrentConeStrongProfileError> {
        let hir_foundation = scoop_hir::OdrFreeHirFoundation::try_new(self.foundation)
            .map_err(CurrentConeStrongProfileError::HirOdr)?;
        Ok(crate::CrossConeStrongIrProductionV1::new(
            hir_foundation,
            self.production_section,
            self.cross_cone_section,
            strong.foundation().clone(),
            strong.production().clone(),
            mir_cross_cone,
            lir,
            lir_cross_cone,
        ))
    }
}
