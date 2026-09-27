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
    let (module, warnings, completion) = lowerer.run_with_dependencies(&files, world)?;
    let output_kind = select_cone_output_kind(&module, requested)?;
    let selected = completion.dependencies.finish();
    let output = finish_output(module, output_kind, warnings, world, Some(&selected))?;
    hir::DependencyHirOutput::try_new(output, selected, completion.binding_witness_uses).map_err(
        |error| {
            vec![Diagnostic::at(
                Span { start: 0, end: 0 },
                format!("failed to seal dependency-aware HIR: {error}"),
            )]
        },
    )
}
