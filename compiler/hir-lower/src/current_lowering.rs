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
    let current_sources = input
        .sources()
        .iter()
        .map(|source| source.text().identity().clone())
        .collect::<Vec<_>>();
    lower_current_cone_inner(requested, input).map_err(|mut diagnostics| {
        for diagnostic in &mut diagnostics {
            diagnostic.resolve_sources(&current_sources);
        }
        diagnostics
    })
}

fn lower_current_cone_inner(
    requested: scoop_identity::RequestedConeKind,
    input: &CurrentConeSources<'_, '_>,
) -> Result<hir::DependencyHirOutput, Vec<Diagnostic>> {
    let (files, sources) = materialize_current_sources(input.sources());
    let world = input.semantic_world();
    let dependency_selection = world.dependency_selection_plan().map_err(|error| {
        vec![Diagnostic::at(
            Span { start: 0, end: 0 },
            format!("failed to prepare dependency selection: {error}"),
        )]
    })?;
    let lowerer = Lowerer::new()
        .with_intrinsic_sources(sources, IntrinsicDeclarationPolicy::CoreOnly)
        .with_imported_dependencies(dependency_selection);
    let lowerer = match input.core() {
        CoreProtocolInput::CurrentDeclarations => lowerer,
        CoreProtocolInput::Imported(core) => lowerer.with_imported_core(core),
    };
    let (module, mut warnings, completion) = lowerer.run_with_dependencies(&files, world)?;
    let output_kind = select_cone_output_kind(&module, requested)?;
    let selected = completion.dependencies.finish();
    let sources = module
        .source_files
        .iter()
        .map(|source| source.identity.clone())
        .collect::<Vec<_>>();
    for warning in &mut warnings {
        warning.resolve_sources(&sources);
    }
    let output = finish_output(module, output_kind, warnings, world, Some(&selected)).map_err(
        |mut diagnostics| {
            for diagnostic in &mut diagnostics {
                diagnostic.resolve_sources(&sources);
            }
            diagnostics
        },
    )?;
    hir::DependencyHirOutput::try_new(output, selected, completion.binding_witness_uses).map_err(
        |error| {
            vec![Diagnostic::at(
                Span { start: 0, end: 0 },
                format!("failed to seal dependency-aware HIR: {error}"),
            )]
        },
    )
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
