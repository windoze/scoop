//! Publish the same finite source projection that supplies concrete protocols.

use super::*;

pub(crate) fn lower_output(
    output: &mut export::ExportHirOutput,
    dependencies: Option<&export::SelectedImportedDependencySet>,
) -> Result<export::LocalConcreteHirOutput, Vec<scoop_ast::Diagnostic>> {
    let automatic = AutomaticNominalRoots::from_output(output).map_err(nominal_root_diagnostic)?;
    let mut concretizer = Concretizer::new(output.module(), automatic);
    let prepared = concretizer.prepare_source();
    let nominals = concretizer.shared_nominal_roots();
    let shared = output
        .shared_source()
        .with_materialized_nominals(output.module(), &nominals, dependencies)
        .map_err(|error| {
            vec![scoop_ast::Diagnostic::without_span(
                scoop_ast::DiagnosticSeverity::Error,
                0,
                format!("failed to project materialized source declarations: {error}"),
            )]
        })?
        .map(std::sync::Arc::new)
        .unwrap_or_else(|| output.shared_source().clone());
    let requirements =
        export::PublicNominalShapeRequirementsV1::from_shared_source(output.module(), &shared)
            .map_err(nominal_root_diagnostic)?;
    concretizer.seed_shape_results(&requirements);
    let (module, output_kind) = match output.output_kind() {
        export::ConeOutputKind::Library => {
            let (module, ()) = concretizer.finish(prepared, |_| ())?;
            (module, export::LocalConeOutputKind::Library)
        }
        export::ConeOutputKind::Executable { local_entry } => {
            let (module, function) = concretizer.finish(prepared, |concretizer| {
                concretizer.intern_type(concrete::TypeKind::Any, false);
                concretizer.function_by_key[&concretizer.function_key(
                    FunctionSource::Local(local_entry.local_function().function()),
                    None,
                    Vec::new(),
                )]
            })?;
            let entry = export::ConcreteExecutableEntry::try_new(&module, local_entry, function)
                .expect("concretization preserves the validated executable entry");
            (
                module,
                export::LocalConeOutputKind::Executable {
                    local_entry: Box::new(entry),
                },
            )
        }
    };
    runtime_exceptions::check_runtime_layout(&module)?;
    let materialization = export::LocalShapeSupportPlan::try_new(&module, &requirements)
        .expect("validated shared shape roots survive concretization");
    output.set_shared_source(shared);
    Ok(
        export::LocalConcreteHirOutput::try_new(module, output_kind, materialization)
            .expect("concretization produces a structurally valid closed output"),
    )
}
