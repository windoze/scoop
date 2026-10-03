//! All production profiles share one physical lowering and complete LIR output.

use super::*;
use scoop_identity::{CanonicalExactTypeDiagnosticName, ExactTypeDiagnosticGraph};

/// Lowers the complete MIR input with its source display diagnostics.
pub fn lower(
    input: &mir::ConeMirInput,
    external_descriptors: &[lir::ExternalTypeDescriptor],
    selected_callables: &lir::SelectedExternalLirSet,
    target_profile: lir::LirTargetProfile,
) -> Result<lir::ConeLirOutput, LirLoweringError> {
    seal(
        input,
        lowering::lower_graph(
            input,
            external_descriptors,
            selected_callables,
            target_profile,
            None,
        )?,
    )
}

/// Layout-profile descriptors use persistent graph spelling before their
/// registration plans, object atoms or export inventories can be produced.
pub fn lower_with_diagnostics(
    input: &mir::ConeMirInput,
    external_descriptors: &[lir::ExternalTypeDescriptor],
    selected_callables: &lir::SelectedExternalLirSet,
    target_profile: lir::LirTargetProfile,
    diagnostics: &impl ExactTypeDiagnosticGraph,
) -> Result<lir::ConeLirOutput, LirLoweringError> {
    let mut lowered = lowering::lower_graph(
        input,
        external_descriptors,
        selected_callables,
        target_profile,
        None,
    )?;
    canonicalize(&mut lowered.module, diagnostics)?;
    seal(input, lowered)
}

fn canonicalize(
    module: &mut lir::Module,
    diagnostics: &impl ExactTypeDiagnosticGraph,
) -> Result<(), LirLoweringError> {
    for (_, descriptor) in module.meta.type_descriptors.iter_mut() {
        descriptor.diagnostic_name = CanonicalExactTypeDiagnosticName::from_validated_graph(
            descriptor.identity.exact_type(),
            diagnostics,
        )
        .map_err(LirLoweringError::Diagnostic)?
        .into_string();
    }
    Ok(())
}

/// Uses the same closed layout selection for actual dependency descriptors
/// and Strong V2 production; imported helpers never become local definitions.
pub fn lower_with_layout_dependencies(
    input: &mir::ConeMirInput,
    selected_callables: &lir::SelectedExternalLirSet,
    target_profile: lir::LirTargetProfile,
    selected_layout: &lir::StrongProductionDependencySelectionV2<'_>,
    diagnostics: &impl ExactTypeDiagnosticGraph,
) -> Result<lir::ConeLirOutput, LirLoweringError> {
    let mut lowered = lowering::lower_graph(
        input,
        &[],
        selected_callables,
        target_profile,
        Some(selected_layout),
    )?;
    canonicalize(&mut lowered.module, diagnostics)?;
    seal(input, lowered)
}

fn seal(
    input: &mir::ConeMirInput,
    lowered: lowering::LoweredModule,
) -> Result<lir::ConeLirOutput, LirLoweringError> {
    lir::ConeLirOutput::try_new(
        lowered.module,
        input
            .materialization()
            .shape_support()
            .iter()
            .map(|root| root.declaration().clone())
            .collect(),
    )
    .map_err(LirLoweringError::Output)
}
