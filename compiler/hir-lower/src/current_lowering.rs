//! Current-Cone source lowering against the shared dependency semantic world.

use crate::pipeline::cancellation::{self, FrontendCancellation, PhaseFailure};
use scoop_ast::{Diagnostic, Span};
use scoop_hir as hir;

use crate::{
    CoreProtocolInput, CurrentConeSources, IntrinsicDeclarationPolicy, Lowerer, finish_output,
    materialize_current_sources, seal_export_output, select_cone_output_kind,
};

/// Lowers current sources against one validated dependency semantic world.
pub fn lower_current_cone(
    requested: scoop_identity::RequestedConeKind,
    input: &CurrentConeSources<'_, '_>,
) -> Result<hir::DependencyHirOutput, Vec<Diagnostic>> {
    lower_current_mode(requested, input, false, None)
}

/// Completes source HIR without creating LocalConcrete HIR or machine instances.
pub fn lower_current_export(
    requested: scoop_identity::RequestedConeKind,
    input: &CurrentConeSources<'_, '_>,
) -> Result<hir::DependencyExportHirOutput, Vec<Diagnostic>> {
    with_current_source_diagnostics(input, || {
        lower_current_export_inner(
            requested,
            input,
            false,
            None,
            None,
            &cancellation::uninterrupted,
        )
        .map_err(PhaseFailure::into_diagnostics)
    })
}

/// Checks a source dependency while allowing its owning editor request to stop.
pub fn lower_current_export_with_cancellation(
    requested: scoop_identity::RequestedConeKind,
    input: &CurrentConeSources<'_, '_>,
    is_cancelled: impl Fn() -> bool + 'static,
) -> Result<hir::DependencyExportHirOutput, crate::EditingAnalysisError> {
    let cancellation = FrontendCancellation::new(is_cancelled);
    let result = lower_current_export_inner(
        requested,
        input,
        false,
        None,
        Some(cancellation.clone()),
        &|| cancellation.check(),
    );
    if cancellation.cancelled() {
        return Err(crate::EditingAnalysisError::Cancelled);
    }
    result.map_err(|failure| match failure {
        PhaseFailure::Cancelled(()) => crate::EditingAnalysisError::Cancelled,
        PhaseFailure::Diagnostics(diagnostics) => crate::EditingAnalysisError::Input(
            resolve_current_source_diagnostics(input, diagnostics),
        ),
    })
}

/// Lowers current sources, asking the driver to fill missing interface bodies.
pub fn lower_current_cone_with_generator(
    requested: scoop_identity::RequestedConeKind,
    input: &CurrentConeSources<'_, '_>,
    generator: &mut dyn hir::MethodBodyGenerator,
) -> Result<hir::DependencyHirOutput, Vec<Diagnostic>> {
    lower_current_mode(requested, input, false, Some(generator))
}

/// Builds the current Cone's tests with a generated executable entry.
pub fn lower_current_tests(
    input: &CurrentConeSources<'_, '_>,
) -> Result<hir::DependencyHirOutput, Vec<Diagnostic>> {
    lower_current_mode(
        scoop_identity::RequestedConeKind::Executable,
        input,
        true,
        None,
    )
}

/// Uses the same generation step while compiling the current Cone's tests.
pub fn lower_current_tests_with_generator(
    input: &CurrentConeSources<'_, '_>,
    generator: &mut dyn hir::MethodBodyGenerator,
) -> Result<hir::DependencyHirOutput, Vec<Diagnostic>> {
    lower_current_mode(
        scoop_identity::RequestedConeKind::Executable,
        input,
        true,
        Some(generator),
    )
}

fn lower_current_mode(
    requested: scoop_identity::RequestedConeKind,
    input: &CurrentConeSources<'_, '_>,
    tests: bool,
    generator: Option<&mut dyn hir::MethodBodyGenerator>,
) -> Result<hir::DependencyHirOutput, Vec<Diagnostic>> {
    with_current_source_diagnostics(input, || {
        let source = lower_current_export_inner(
            requested,
            input,
            tests,
            generator,
            None,
            &cancellation::uninterrupted,
        )
        .map_err(PhaseFailure::into_diagnostics)?;
        let sources = source
            .export
            .source_files
            .iter()
            .map(|source| source.identity.clone())
            .collect::<Vec<_>>();
        let mut output = finish_output(
            source.export,
            source.warnings,
            input.semantic_world(),
            Some(&source.imported_dependencies),
        )
        .map_err(|mut diagnostics| {
            for diagnostic in &mut diagnostics {
                diagnostic.resolve_sources(&sources);
            }
            diagnostics
        })?;
        output.documentation = source.documentation;
        hir::DependencyHirOutput::try_new(
            output,
            source.imported_dependencies,
            source.binding_witness_uses,
        )
        .map_err(|error| {
            vec![Diagnostic::at(
                Span { start: 0, end: 0 },
                format!("failed to seal dependency-aware HIR: {error}"),
            )]
        })
    })
}

fn with_current_source_diagnostics<T>(
    input: &CurrentConeSources<'_, '_>,
    run: impl FnOnce() -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    run().map_err(|diagnostics| resolve_current_source_diagnostics(input, diagnostics))
}

fn resolve_current_source_diagnostics(
    input: &CurrentConeSources<'_, '_>,
    mut diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    let current_sources = input
        .sources()
        .iter()
        .map(|source| source.text().identity().clone())
        .collect::<Vec<_>>();
    for diagnostic in &mut diagnostics {
        diagnostic.resolve_sources(&current_sources);
    }
    diagnostics
}

fn lower_current_export_inner<C>(
    requested: scoop_identity::RequestedConeKind,
    input: &CurrentConeSources<'_, '_>,
    tests: bool,
    generator: Option<&mut dyn hir::MethodBodyGenerator>,
    cancellation: Option<FrontendCancellation>,
    check: &impl Fn() -> Result<(), C>,
) -> Result<hir::DependencyExportHirOutput, PhaseFailure<C>> {
    PhaseFailure::check(check)?;
    let (files, mut sources) = materialize_current_sources(input.sources());
    for source in &mut sources {
        if let Some(coordinate) = input.source_names().get(&source.identity.cone()) {
            source.name = format!("{coordinate}/{}", source.identity.logical_path().as_str());
        }
    }
    let world = input.semantic_world();
    let mut lowerer = prepare_current_lowerer(sources, world)?;
    lowerer.cancellation = cancellation;
    lowerer.source_names = input.source_names().clone();
    lowerer.test_dependencies = input.test_dependencies().clone();
    lowerer.documentation = input
        .documentation()
        .map(|sources| crate::documentation::DocumentationContext::new(sources).into());
    if tests {
        lowerer.test_compilation = Some(crate::test_entry::TestCompilation::new(world));
    }
    let lowerer = match input.core() {
        CoreProtocolInput::CurrentDeclarations => lowerer,
        CoreProtocolInput::Imported(core) => lowerer.with_imported_core(core),
    };
    let (module, mut warnings, completion) =
        lowerer.run_with_dependencies(&files, world, generator, check)?;
    PhaseFailure::check(check)?;
    let output_kind = if tests {
        crate::test_entry::output_kind(&module)?
    } else {
        select_cone_output_kind(&module, requested)?
    };
    let selected = completion.dependencies.finish();
    let sources = module
        .source_files
        .iter()
        .map(|source| source.identity.clone())
        .collect::<Vec<_>>();
    for warning in &mut warnings {
        warning.resolve_sources(&sources);
    }
    let export =
        seal_export_output(module, output_kind, Some(&selected)).map_err(|mut diagnostics| {
            for diagnostic in &mut diagnostics {
                diagnostic.resolve_sources(&sources);
            }
            diagnostics
        })?;
    PhaseFailure::check(check)?;
    Ok(hir::DependencyExportHirOutput {
        export,
        imported_dependencies: selected,
        binding_witness_uses: completion.binding_witness_uses,
        warnings,
        documentation: completion.documentation,
    })
}

pub(crate) fn prepare_current_lowerer(
    sources: Vec<crate::SourceProvider>,
    world: &hir::ImportedSemanticWorld<'_>,
) -> Result<Lowerer, Vec<Diagnostic>> {
    let dependency_selection = world.dependency_selection_plan().map_err(|error| {
        vec![Diagnostic::at(
            Span { start: 0, end: 0 },
            format!("failed to prepare dependency selection: {error}"),
        )]
    })?;
    Ok(Lowerer::new()
        .with_intrinsic_sources(sources, IntrinsicDeclarationPolicy::CoreOnly)
        .with_imported_dependencies(dependency_selection))
}

impl Lowerer {
    pub(crate) fn diagnostic_source_identities(&self) -> Vec<scoop_identity::SourceIdentity> {
        self.intrinsic_sources
            .iter()
            .map(|source| source.identity.clone())
            .chain(
                self.imported_source_files
                    .iter()
                    .map(|source| source.identity.clone()),
            )
            .collect()
    }

    pub(crate) fn take_source_diagnostics(&mut self) -> Vec<Diagnostic> {
        let sources = self.diagnostic_source_identities();
        let mut diagnostics = std::mem::take(&mut self.diagnostics);
        for diagnostic in &mut diagnostics {
            diagnostic.resolve_sources(&sources);
        }
        diagnostics
    }
}
