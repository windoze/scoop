//! Complete HIR products shared by core and ordinary Cones.

mod errors;
mod test_directory;
pub use errors::CurrentConeHirStageError;

pub(super) struct CurrentConeHirArtifacts {
    pub test_directory: Option<scoop_slib::TestDirectory>,
    pub hir: scoop_hir::DependencyHirOutput,
    pub foundation: scoop_hir::CanonicalHirFoundation,
    pub production_section: scoop_hir::CoreBootstrapInterfaceSectionV1,
    pub cross_cone_section: scoop_hir::CrossConeHirInterfaceSectionV1,
    pub nominal_classifier: scoop_hir::NominalExactLeafClassifierV1,
    pub generated_sources: Vec<scoop_hir::GeneratedMethodSource>,
    pub generator_warnings: Vec<scoop_ast::Diagnostic>,
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
        Self::lower_with_documentation(
            requested,
            sources,
            protocols,
            world,
            source_names,
            false,
            &scoop_toolchain::GeneratorPrograms::default(),
        )
    }

    pub(super) fn lower_with_documentation(
        requested: scoop_identity::RequestedConeKind,
        parsed: &scoop_ast::CurrentConeParsedSources,
        protocols: scoop_hir_lower::CoreProtocolInput,
        world: &scoop_hir::ImportedSemanticWorld<'_>,
        source_names: std::collections::BTreeMap<scoop_identity::ConeIdentity, String>,
        documentation: bool,
        programs: &scoop_toolchain::GeneratorPrograms,
    ) -> Result<Self, CurrentConeHirStageError> {
        let mut sources = scoop_hir_lower::CurrentConeSources::try_new(parsed, protocols, world)
            .map_err(CurrentConeHirStageError::Input)?
            .with_source_names(source_names);
        if documentation {
            sources = sources
                .with_documentation(
                    parsed
                        .iter()
                        .map(|source| scoop_ast::IdentifiedSourceDocumentation {
                            source: source.source().identity().clone(),
                            documentation: scoop_doc::prepare_source(
                                source.source().ast(),
                                source.text().text(),
                            ),
                        })
                        .collect(),
                )
                .map_err(CurrentConeHirStageError::Input)?;
        }
        let mut generator = crate::generation::ExternalGenerator::new(programs);
        let result =
            scoop_hir_lower::lower_current_cone_with_generator(requested, &sources, &mut generator);
        let hir = finish_lowering(result, parsed, &mut generator)?;
        let mut artifacts = Self::from_hir(hir, world)?;
        artifacts.generated_sources = generator.sources;
        artifacts.generator_warnings = generator.warnings;
        Ok(artifacts)
    }

    pub fn lower_tests(
        sources: &scoop_ast::CurrentConeParsedSources,
        protocols: scoop_hir_lower::CoreProtocolInput,
        world: &scoop_hir::ImportedSemanticWorld<'_>,
        source_names: std::collections::BTreeMap<scoop_identity::ConeIdentity, String>,
        test_dependencies: std::collections::BTreeSet<scoop_identity::ConeIdentity>,
        programs: &scoop_toolchain::GeneratorPrograms,
    ) -> Result<Self, CurrentConeHirStageError> {
        let parsed_sources = sources;
        let sources = scoop_hir_lower::CurrentConeSources::try_new(sources, protocols, world)
            .map_err(CurrentConeHirStageError::Input)?
            .with_source_names(source_names)
            .with_test_dependencies(test_dependencies)
            .map_err(CurrentConeHirStageError::Input)?;
        let mut generator = crate::generation::ExternalGenerator::new(programs);
        let result = scoop_hir_lower::lower_current_tests_with_generator(&sources, &mut generator);
        let hir = finish_lowering(result, parsed_sources, &mut generator)?;
        let directory =
            test_directory::project(&hir.output().export.test_containers, parsed_sources);
        let mut artifacts = Self::from_hir(hir, world)?;
        artifacts.test_directory = Some(directory);
        artifacts.generated_sources = generator.sources;
        artifacts.generator_warnings = generator.warnings;
        Ok(artifacts)
    }

    fn from_hir(
        hir: scoop_hir::DependencyHirOutput,
        world: &scoop_hir::ImportedSemanticWorld<'_>,
    ) -> Result<Self, CurrentConeHirStageError> {
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
            generated_sources: Vec::new(),
            generator_warnings: Vec::new(),
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

fn finish_lowering(
    result: Result<scoop_hir::DependencyHirOutput, Vec<scoop_ast::Diagnostic>>,
    parsed: &scoop_ast::CurrentConeParsedSources,
    generator: &mut crate::generation::ExternalGenerator<'_>,
) -> Result<scoop_hir::DependencyHirOutput, CurrentConeHirStageError> {
    result.map_err(|mut diagnostics| {
        diagnostics.append(&mut generator.warnings);
        match crate::CurrentConeDiagnosticSet::try_new_with_generated(
            diagnostics,
            parsed,
            &generator.sources,
        ) {
            Ok(diagnostics) => CurrentConeHirStageError::Lowering(diagnostics),
            Err(error) => CurrentConeHirStageError::Diagnostics(error),
        }
    })
}
