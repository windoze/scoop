use scoop_identity::{CallableOwner, Effect, StrongCallableDefinitionOwner};
use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};

use super::super::lir_callable_layouts::{Layouts, clone_signature};
use super::SharedLirInitializationAbiValidationError as Error;

/// The MIR role and signature must already be joined to the same artifact's
/// HIR protocol declaration. Dependencies are its validated reachable layouts.
pub fn replay_shared_initialization_abi(
    target: lir::LirTargetProfile,
    mir: &mir::StrongCallableBridgeSurfaceV1,
    local: &lir::CanonicalExactLayoutExportsV1,
    dependencies: &[&lir::CanonicalExactLayoutExportsV1],
    foundation: &lir::OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<Option<Box<lir::CallableAbiRecordV1>>, Error> {
    let layouts = Layouts::new(local, dependencies, target, foundation.producer(), meter)?;
    let path = WirePath::root();
    meter.charge_work(mir.bridges().len() as u64, &path)?;
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
        .signature(signature, meter)
        .map_err(Error::SignatureLayouts)?;
    let abi = lir::CallableAbiRecordV1::replay(
        target,
        StrongCallableDefinitionOwner::Function(function),
        clone_signature(signature, meter)?,
        lir::ExactCallableProtocolV1::OrdinaryManaged,
        inputs.inputs(),
        foundation,
        meter,
    )?;
    meter.charge_owned_bytes(
        std::mem::size_of::<lir::CallableAbiRecordV1>() as u64,
        &path,
    )?;
    Ok(Some(Box::new(abi)))
}
