//! Current-Cone source lowering against the shared dependency semantic world.

use scoop_ast::{Diagnostic, Span};
use scoop_hir as hir;

use crate::{
    CoreProtocolInput, CurrentConeSources, IntrinsicDeclarationPolicy, Lowerer, finish_output,
    materialize_current_sources, select_cone_output_kind,
};

/// Lowers current sources against one validated dependency semantic world.
pub fn lower_current_cone(
    requested: scoop_identity::RequestedConeKind,
    input: &CurrentConeSources<'_, '_>,
) -> Result<hir::DependencyHirOutput, Vec<Diagnostic>> {
    lower_current_mode(requested, input, false, None)
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
    let current_sources = input
        .sources()
        .iter()
        .map(|source| source.text().identity().clone())
        .collect::<Vec<_>>();
    lower_current_cone_inner(requested, input, tests, generator).map_err(|mut diagnostics| {
        for diagnostic in &mut diagnostics {
            diagnostic.resolve_sources(&current_sources);
        }
        diagnostics
    })
}

fn lower_current_cone_inner(
    requested: scoop_identity::RequestedConeKind,
    input: &CurrentConeSources<'_, '_>,
    tests: bool,
    generator: Option<&mut dyn hir::MethodBodyGenerator>,
) -> Result<hir::DependencyHirOutput, Vec<Diagnostic>> {
    let (files, mut sources) = materialize_current_sources(input.sources());
    for source in &mut sources {
        if let Some(coordinate) = input.source_names().get(&source.identity.cone()) {
            source.name = format!("{coordinate}/{}", source.identity.logical_path().as_str());
        }
    }
    let world = input.semantic_world();
    let mut lowerer = prepare_current_lowerer(sources, world)?;
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
        lowerer.run_with_dependencies(&files, world, generator)?;
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
    let mut output = finish_output(module, output_kind, warnings, world, Some(&selected)).map_err(
        |mut diagnostics| {
            for diagnostic in &mut diagnostics {
                diagnostic.resolve_sources(&sources);
            }
            diagnostics
        },
    )?;
    output.documentation = completion.documentation;
    hir::DependencyHirOutput::try_new(output, selected, completion.binding_witness_uses).map_err(
        |error| {
            vec![Diagnostic::at(
                Span { start: 0, end: 0 },
                format!("failed to seal dependency-aware HIR: {error}"),
            )]
        },
    )
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
