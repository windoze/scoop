use scoop_lir as lir;
use scoop_mir as mir;
use scoop_wire::{BudgetMeter, WirePath};

use super::SharedLirCallableAbiValidationError as Error;

mod layouts;
use layouts::Layouts;

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
    let path = WirePath::root();
    let exact = signature.exact();
    meter.charge_work(exact.parameters().len() as u64 + 2, &path)?;
    meter.charge_edges(exact.parameters().len() as u64 + 2, &path)?;
    let receiver = match exact.receiver().into_option() {
        None => lir::CallableAbiReceiverInputV1::NoReceiver,
        Some(exact) => lir::CallableAbiReceiverInputV1::Receiver(layouts.value(exact, meter)?),
    };
    let mut parameters = Vec::new();
    meter.try_reserve_collection_slots(&mut parameters, exact.parameters().len(), &path)?;
    for exact in exact.parameters() {
        parameters.push(layouts.value(*exact, meter)?);
    }
    let result = layouts.value(exact.result(), meter)?;
    let protocol = match signature.gc_effect() {
        mir::GcEffect::Managed => lir::ExactCallableProtocolV1::OrdinaryManaged,
        mir::GcEffect::NoGc => lir::ExactCallableProtocolV1::OrdinaryNoGc,
    };
    meter.charge_collection_slots(exact.parameters().len() as u64, &path)?;
    meter.charge_owned_bytes(
        (exact.parameters().len() as u64)
            .saturating_mul(std::mem::size_of::<scoop_identity::PersistentExactTypeId>() as u64),
        &path,
    )?;
    lir::ExactCallableAbiExportV1::replay(
        target,
        implementation,
        exact.clone(),
        protocol,
        lir::CallableAbiLayoutInputsV1 {
            receiver,
            parameters: &parameters,
            result,
        },
        foundation,
        meter,
    )
}
