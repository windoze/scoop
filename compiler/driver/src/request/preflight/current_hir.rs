//! Complete HIR products shared by core and ordinary Cones.

mod errors;
pub use errors::CurrentConeHirStageError;

pub(super) struct CurrentConeHirArtifacts {
    pub hir: scoop_hir::DependencyHirOutput,
    pub foundation: scoop_hir::CanonicalHirFoundation,
    pub production_section: scoop_hir::CoreBootstrapInterfaceSectionV1,
    pub cross_cone_section: scoop_hir::CrossConeHirInterfaceSectionV1,
    pub nominal_classifier: scoop_hir::NominalExactLeafClassifierV1,
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

        let mut foundation = scoop_hir::CanonicalHirFoundation::from_type_semantics_output(&hir)
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
        let nominal_classifier = world
            .nominal_exact_leaf_classifier(cross_cone_section.nominal_interfaces())
            .map_err(CurrentConeHirStageError::NominalClassifier)?;
        foundation
            .complete_cross_cone_interface_source_points(
                hir.output().export.module(),
                &cross_cone_section,
            )
            .map_err(CurrentConeHirStageError::Foundation)?;
        Ok(Self {
            hir,
            foundation,
            production_section,
            cross_cone_section,
            nominal_classifier,
        })
    }

    pub fn machine_input(&self) -> super::machine::CurrentConeMachineHir<'_> {
        super::machine::CurrentConeMachineHir {
            output: &self.hir,
            production: &self.production_section,
            public: &self.cross_cone_section,
            classifier: &self.nominal_classifier,
        }
    }
}
