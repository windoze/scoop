use super::*;

pub(in crate::request::preflight) struct CurrentConeHir {
    pub hir: scoop_hir::DependencyHirOutput,
    pub generated_sources: Vec<scoop_hir::GeneratedMethodSource>,
    pub generator_warnings: Vec<scoop_ast::Diagnostic>,
    pub used_generators: Vec<scoop_protocol::GeneratorToolInputV1>,
    pub compiler_inputs: Vec<scoop_protocol::CompilerInputV1>,
}

impl CurrentConeHir {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::request::preflight) fn lower_with_documentation(
        requested: scoop_identity::RequestedConeKind,
        parsed: &scoop_ast::CurrentConeParsedSources,
        protocols: scoop_hir_lower::CoreProtocolInput,
        world: &scoop_hir::ImportedSemanticWorld<'_>,
        source_names: std::collections::BTreeMap<scoop_identity::ConeIdentity, String>,
        documentation: bool,
        programs: &scoop_toolchain::GeneratorPrograms,
        inputs: std::sync::Arc<scoop_toolchain::CompileTimeInputs>,
    ) -> Result<Self, CurrentConeHirStageError> {
        let mut sources = scoop_hir_lower::CurrentConeSources::try_new(parsed, protocols, world)
            .map_err(CurrentConeHirStageError::Input)?
            .with_source_names(source_names)
            .with_compile_time_inputs(inputs.clone());
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
        Ok(Self {
            hir,
            generated_sources: generator.sources,
            generator_warnings: generator.warnings,
            used_generators: generator.used,
            compiler_inputs: inputs.compiler_inputs(),
        })
    }

    pub fn lower_tests(
        sources: &scoop_ast::CurrentConeParsedSources,
        protocols: scoop_hir_lower::CoreProtocolInput,
        world: &scoop_hir::ImportedSemanticWorld<'_>,
        source_names: std::collections::BTreeMap<scoop_identity::ConeIdentity, String>,
        test_dependencies: std::collections::BTreeSet<scoop_identity::ConeIdentity>,
        programs: &scoop_toolchain::GeneratorPrograms,
        inputs: std::sync::Arc<scoop_toolchain::CompileTimeInputs>,
    ) -> Result<Self, CurrentConeHirStageError> {
        let parsed_sources = sources;
        let sources = scoop_hir_lower::CurrentConeSources::try_new(sources, protocols, world)
            .map_err(CurrentConeHirStageError::Input)?
            .with_source_names(source_names)
            .with_compile_time_inputs(inputs.clone())
            .with_test_dependencies(test_dependencies)
            .map_err(CurrentConeHirStageError::Input)?;
        let mut generator = crate::generation::ExternalGenerator::new(programs);
        let result = scoop_hir_lower::lower_current_tests_with_generator(&sources, &mut generator);
        let hir = finish_lowering(result, parsed_sources, &mut generator)?;
        Ok(Self {
            hir,
            generated_sources: generator.sources,
            generator_warnings: generator.warnings,
            used_generators: generator.used,
            compiler_inputs: inputs.compiler_inputs(),
        })
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
