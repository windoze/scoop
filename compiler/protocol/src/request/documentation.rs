use super::*;

pub(super) fn validate_documentation(
    current: &CurrentConeRequestV1,
    purpose: crate::CompilationPurpose,
    output: &ScoopcOutputRequestV1,
    emit: &StageDumpPolicyV1,
) -> Result<(), ProtocolValidationError> {
    if output.documentation().is_none() {
        return Ok(());
    }
    if matches!(current, CurrentConeRequestV1::SingleFile { .. })
        || purpose != crate::CompilationPurpose::Production
    {
        return Err(ProtocolValidationError::DocumentationRequiresProductionManifest);
    }
    if output.artifact().is_none()
        && let StageDumpPolicyV1::Files { stages, .. } = emit
        && (stages.contains(crate::StageDumpKindV1::Mir)
            || stages.contains(crate::StageDumpKindV1::Optimizations)
            || stages.contains(crate::StageDumpKindV1::Lir))
    {
        return Err(ProtocolValidationError::DocumentationMachineDump);
    }
    Ok(())
}
