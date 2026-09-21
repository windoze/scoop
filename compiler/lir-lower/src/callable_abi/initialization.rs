use super::*;

pub(crate) fn lower_initialization_abi(
    input: &mir::SingleConeStrongMirInput,
    functions: &[lir::Function],
    enums: &lir::EnumDefs,
) -> Result<lir::CoreLirBridgeBranchV1, crate::StrongLirLoweringError> {
    let Some(cycle) = input
        .production()
        .strong_callable_bridges()
        .initialization_cycle()
    else {
        return Ok(lir::CoreLirBridgeBranchV1::NotCore);
    };
    let scoop_identity::CallableOwner::Function(function) = cycle.implementation() else {
        return Err(
            crate::StrongLirLoweringError::InvalidInitializationCallable(cycle.implementation()),
        );
    };
    if cycle.signature().effect() != scoop_identity::Effect::Ordinary
        || cycle.signature().receiver().is_present()
    {
        return Err(
            crate::StrongLirLoweringError::InvalidInitializationCallable(cycle.implementation()),
        );
    }
    let target = StrongCallableDefinitionOwner::Function(function);
    let record = LocalCallableMaterialization::resolve(input, functions, target, cycle.signature())
        .and_then(|body| body.abi_record(enums))
        .map_err(crate::StrongLirLoweringError::CallableAbi)?;
    Ok(lir::CoreLirBridgeBranchV1::Core(lir::CoreLirBridgeV1::new(
        record,
    )))
}
