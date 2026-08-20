use super::*;

pub(super) fn resolve(
    view: &NativeBoundaryFoundationView<'_>,
    dependencies: &[AbiReplayDependency<'_>],
) -> Result<Vec<SourceNativeExternalContractRecord>, NativeBoundaryCompileError> {
    let mut sources = view.source_contracts.to_vec();
    for target in view.native_contracts {
        if sources.iter().any(|source| source.id() == target.source()) {
            continue;
        }
        let source = dependencies
            .iter()
            .flat_map(|dependency| dependency.foundation.source_native_contracts())
            .find(|source| source.id() == target.source())
            .ok_or(NativeBoundaryTargetError::NativeContractMismatch)?;
        sources.push(source.clone());
    }
    Ok(sources)
}
