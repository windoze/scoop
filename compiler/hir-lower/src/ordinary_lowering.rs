//! Ordinary source lowering against the shared dependency semantic world.

use scoop_ast::{Diagnostic, Span};
use scoop_hir as hir;

use crate::{
    IntrinsicDeclarationPolicy, Lowerer, OrdinarySources, concretize, materialize_ordinary_sources,
    native_boundary_diagnostic, select_cone_output_kind,
};

/// Lowers current sources against one validated dependency semantic world.
pub fn lower_ordinary(
    requested: scoop_identity::RequestedConeKind,
    input: &OrdinarySources<'_, '_>,
) -> Result<hir::OrdinaryHirOutput, Vec<Diagnostic>> {
    let (files, sources) = materialize_ordinary_sources(input.sources());
    let world = input.semantic_world();
    let nominals = world
        .direct_provider(scoop_identity::ConeIdentity::CORE)
        .map(|provider| provider.nominal_interfaces().records())
        .unwrap_or_default();
    let classifier = hir::CoreClosedExactLeafClassifierV1::try_from_nominal_interfaces(nominals)
        .map_err(|error| {
            vec![Diagnostic::at(
                Span { start: 0, end: 0 },
                format!("failed to classify core ABI leaves: {error}"),
            )]
        })?;
    let dependency_selection = world
        .dependency_selection_plan(&classifier)
        .map_err(|error| {
            vec![Diagnostic::at(
                Span { start: 0, end: 0 },
                format!("failed to prepare dependency selection: {error}"),
            )]
        })?;
    let (module, warnings, completion) = Lowerer::new()
        .with_intrinsic_sources(sources, IntrinsicDeclarationPolicy::CoreOnly)
        .with_imported_core(input.core())
        .with_imported_dependencies(dependency_selection)
        .run_imported(&files, world)?;
    let output_kind = select_cone_output_kind(&module, requested)?;
    let export = hir::ExportHirOutput::try_new(module, output_kind).map_err(|error| {
        vec![Diagnostic::at(
            Span { start: 0, end: 0 },
            format!("failed to seal ordinary Export HIR output: {error}"),
        )]
    })?;
    let local = concretize::lower_output(&export);
    let native_boundary_types = crate::persistent_native_boundary::build(
        export.module(),
        local.module(),
        hir::HirNativeBoundaryExternalTypes::TrustedCore(input.core().native_boundary_types()),
    )
    .map_err(native_boundary_diagnostic)?;
    let output =
        hir::Output::try_new(export, local, native_boundary_types, warnings).map_err(|error| {
            vec![Diagnostic::at(
                Span { start: 0, end: 0 },
                format!("failed to seal ordinary HIR output: {error}"),
            )]
        })?;
    hir::OrdinaryHirOutput::try_new(
        output,
        completion.dependencies.finish(),
        completion.binding_witness_uses,
    )
    .map_err(|error| {
        vec![Diagnostic::at(
            Span { start: 0, end: 0 },
            format!("failed to seal ordinary imported-core HIR: {error}"),
        )]
    })
}
