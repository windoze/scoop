use scoop_identity::{CallableOwner, Effect, StrongCallableDefinitionOwner};
use scoop_lir as lir;
use scoop_mir as mir;

use super::super::lir_callable_layouts::Layouts;
use super::SharedLirInitializationAbiValidationError as Error;

/// The MIR role and signature must already be joined to the same artifact's
/// HIR protocol declaration. Dependencies are its validated reachable layouts.
pub fn replay_shared_initialization_abi(
    target: lir::LirTargetProfile,
    mir: &mir::StrongCallableBridgeSurfaceV1,
    local: &lir::CanonicalExactLayoutExportsV1,
    dependencies: &[&lir::CanonicalExactLayoutExportsV1],
    foundation: &lir::OdrFreeLirFoundation,
) -> Result<Option<Box<lir::CallableAbiRecordV1>>, Error> {
    let layouts = Layouts::new(local, dependencies, target, foundation.producer())?;

    let Some(callable) = mir.initialization_cycle() else {
        return Ok(None);
    };
    let CallableOwner::Function(function) = callable.implementation() else {
        return Err(Error::InitializationTarget);
    };
    let signature = callable.signature();
    if signature.effect() != Effect::Ordinary
        || signature.receiver().into_option().is_some()
        || signature.parameters().len() != 1
        || signature.result() != mir::core_unit_exact_type()
    {
        return Err(Error::InitializationSignature);
    }
    let inputs = layouts
        .signature(signature)
        .map_err(Error::SignatureLayouts)?;
    let abi = lir::CallableAbiRecordV1::replay(
        target,
        StrongCallableDefinitionOwner::Function(function),
        (signature).clone(),
        lir::ExactCallableProtocolV1::OrdinaryManaged,
        inputs.inputs(),
        foundation,
    )?;

    Ok(Some(Box::new(abi)))
}
