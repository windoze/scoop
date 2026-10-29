//! Complete HIR products shared by core and ordinary Cones.

mod errors;
mod lowering;
pub(super) use lowering::CurrentConeHir;
pub(super) mod test_directory;
pub use errors::CurrentConeHirStageError;

pub(super) struct CurrentConeHirArtifacts {
    pub test_directory: Option<scoop_slib::TestDirectory>,
    pub hir: scoop_hir::DependencyHirOutput,
    pub foundation: scoop_hir::CanonicalHirFoundation,
    pub production_section: scoop_hir::CoreBootstrapInterfaceSectionV1,
    pub cross_cone_section: scoop_hir::CrossConeHirInterfaceSectionV1,
    pub nominal_classifier: scoop_hir::NominalExactLeafClassifierV1,
}

impl CurrentConeHirArtifacts {
    pub fn machine_dependency_providers(
        &self,
    ) -> impl Iterator<Item = scoop_identity::ConeIdentity> + '_ {
        let current = self.hir.output().export.cone;
        self.cross_cone_section
            .external_references()
            .records()
            .iter()
            .map(scoop_hir::ExternalHirReferenceV1::origin)
            .chain(self.hir.executable_dependency_providers())
            .chain(
                (current != scoop_identity::ConeIdentity::CORE)
                    .then_some(scoop_identity::ConeIdentity::CORE),
            )
    }

    #[cfg(test)]
    pub fn lower(
        requested: scoop_identity::RequestedConeKind,
        sources: &scoop_ast::CurrentConeParsedSources,
        protocols: scoop_hir_lower::CoreProtocolInput,
        world: &scoop_hir::ImportedSemanticWorld<'_>,
        source_names: std::collections::BTreeMap<scoop_identity::ConeIdentity, String>,
    ) -> Result<Self, CurrentConeHirStageError> {
        let source = CurrentConeHir::lower_with_documentation(
            requested,
            sources,
            protocols,
            world,
            source_names,
            false,
            &scoop_toolchain::GeneratorPrograms::default(),
            std::sync::Arc::new(scoop_toolchain::CompileTimeInputs::default()),
        )?;
        Self::from_hir(source, world)
    }

    pub(super) fn from_hir(
        source: CurrentConeHir,
        world: &scoop_hir::ImportedSemanticWorld<'_>,
    ) -> Result<Self, CurrentConeHirStageError> {
        let CurrentConeHir { hir, .. } = source;
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
            test_directory: None,
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
