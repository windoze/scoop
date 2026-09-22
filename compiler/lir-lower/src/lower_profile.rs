//! Profile finalization shares one physical lowering and one strong sealer.

use super::*;
use scoop_identity::{CanonicalExactTypeDiagnosticName, ExactTypeDiagnosticGraph};
use scoop_wire::BudgetMeter;

/// Lowers the existing strong profile with its source display diagnostics.
pub fn lower(
    input: &mir::SingleConeStrongMirInput,
    runtime_string: RuntimeStringDescriptor,
    selected_callables: &lir::SelectedExternalLirSet,
    target_profile: lir::LirTargetProfile,
) -> Result<lir::SingleConeStrongLirOutput, StrongLirLoweringError> {
    seal(
        input,
        lowering::lower_graph(input, runtime_string, selected_callables, target_profile)?,
    )
}

/// Layout-profile descriptors use persistent graph spelling before their
/// registration plans, object atoms or export inventories can be produced.
pub fn lower_with_diagnostics(
    input: &mir::SingleConeStrongMirInput,
    runtime_string: RuntimeStringDescriptor,
    selected_callables: &lir::SelectedExternalLirSet,
    target_profile: lir::LirTargetProfile,
    diagnostics: &impl ExactTypeDiagnosticGraph,
    meter: &mut BudgetMeter,
) -> Result<lir::SingleConeStrongLirOutput, StrongLirLoweringError> {
    let mut lowered =
        lowering::lower_graph(input, runtime_string, selected_callables, target_profile)?;
    for (_, descriptor) in lowered.module.meta.type_descriptors.iter_mut() {
        descriptor.diagnostic_name =
            CanonicalExactTypeDiagnosticName::from_validated_graph_metered(
                descriptor.identity.exact_type(),
                diagnostics,
                meter,
            )
            .map_err(StrongLirLoweringError::Diagnostic)?
            .into_string();
    }
    seal(input, lowered)
}

fn seal(
    input: &mir::SingleConeStrongMirInput,
    lowered: lowering::LoweredModule,
) -> Result<lir::SingleConeStrongLirOutput, StrongLirLoweringError> {
    lir::SingleConeStrongLirOutput::try_new(
        lowered.module,
        input
            .materialization()
            .shape_support()
            .iter()
            .map(|root| root.declaration().clone())
            .collect(),
        lowered.initialization_cycle_abi,
    )
    .map_err(StrongLirLoweringError::Output)
}
