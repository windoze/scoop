use scoop_identity::ExactTypeDiagnosticGraph;
use scoop_lir as lir;
use scoop_lir_lower::{LirLoweringError, StorageLoweringError, prepare};
use scoop_mir as mir;

#[cfg(test)]
pub(crate) fn lower(
    input: &mir::ConeMirInput,
    descriptors: &[lir::ExternalTypeDescriptor],
    callables: &lir::SelectedExternalLirSet,
    target: lir::LirTargetProfile,
) -> Result<lir::ConeLirOutput, LirLoweringError> {
    let prepared = prepare(input, descriptors, callables, target, None)?;
    let plans =
        scoop_codegen::plan_abi(prepared.abi_input()).map_err(StorageLoweringError::from)?;
    prepared.complete(plans)
}

#[cfg(test)]
pub(crate) fn lower_with_diagnostics(
    input: &mir::ConeMirInput,
    descriptors: &[lir::ExternalTypeDescriptor],
    callables: &lir::SelectedExternalLirSet,
    target: lir::LirTargetProfile,
    diagnostics: &impl ExactTypeDiagnosticGraph,
) -> Result<lir::ConeLirOutput, LirLoweringError> {
    let prepared = prepare(input, descriptors, callables, target, None)?;
    let plans =
        scoop_codegen::plan_abi(prepared.abi_input()).map_err(StorageLoweringError::from)?;
    prepared.complete_with_diagnostics(plans, diagnostics)
}

pub(crate) fn lower_with_layout_dependencies(
    input: &mir::ConeMirInput,
    callables: &lir::SelectedExternalLirSet,
    target: lir::LirTargetProfile,
    layouts: &lir::StrongProductionDependencySelectionV2<'_>,
    diagnostics: &impl ExactTypeDiagnosticGraph,
) -> Result<lir::ConeLirOutput, LirLoweringError> {
    let prepared = prepare(input, &[], callables, target, Some(layouts))?;
    let plans =
        scoop_codegen::plan_abi(prepared.abi_input()).map_err(StorageLoweringError::from)?;
    prepared.complete_with_diagnostics(plans, diagnostics)
}
