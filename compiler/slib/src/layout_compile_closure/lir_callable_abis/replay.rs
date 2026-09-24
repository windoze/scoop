use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};

use super::SharedLirCallableAbiValidationError as Error;

use super::super::lir_callable_layouts::{Layouts, clone_signature};

/// Replays the complete ABI constituent from HIR-joined MIR bindings and
/// already checked layouts. This does not authorize a selected use or import.
pub fn replay_shared_mir_callable_abis(
    target: lir::LirTargetProfile,
    bindings: &mir::CanonicalMirCallableBindingsV1,
    local: &lir::CanonicalExactLayoutExportsV1,
    dependencies: &[&lir::CanonicalExactLayoutExportsV1],
    foundation: &lir::OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<lir::CanonicalExactCallableAbiExportsV1, Error> {
    let layouts = Layouts::new(local, dependencies, target, foundation.producer(), meter)?;
    let path = WirePath::root();
    meter.check_table_entries(bindings.entries().len() as u64, &path)?;
    let mut records = Vec::new();
    meter.try_reserve_collection_slots(&mut records, bindings.entries().len(), &path)?;
    for binding in bindings.entries() {
        let implementation = binding.implementation();
        let record = replay(
            target,
            implementation,
            binding.lowered_signature(),
            &layouts,
            foundation,
            meter,
        )
        .map_err(|source| Error::Callable {
            target: implementation,
            source: Box::new(source),
        })?;
        records.push(record);
    }
    Ok(lir::CanonicalExactCallableAbiExportsV1::try_new(
        target, foundation, records, meter,
    )?)
}

fn replay(
    target: lir::LirTargetProfile,
    implementation: scoop_identity::StrongCallableDefinitionOwner,
    signature: &mir::MirBridgeCallableSignatureV1,
    layouts: &Layouts<'_>,
    foundation: &lir::OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<lir::ExactCallableAbiExportV1, lir::ExactCallableAbiError> {
    let exact = signature.exact();
    let inputs = layouts.signature(exact, meter)?;
    let protocol = match signature.gc_effect() {
        mir::GcEffect::Managed => lir::ExactCallableProtocolV1::OrdinaryManaged,
        mir::GcEffect::NoGc => lir::ExactCallableProtocolV1::OrdinaryNoGc,
    };
    lir::ExactCallableAbiExportV1::replay(
        target,
        implementation,
        clone_signature(exact, meter)?,
        protocol,
        inputs.inputs(),
        foundation,
        meter,
    )
}
