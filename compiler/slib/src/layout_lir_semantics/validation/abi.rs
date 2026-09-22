use scoop_identity::{GcEffect, PersistentExactTypeId, RepresentationRole};
use scoop_lir::{
    CallableAbiLayoutInputsV1, CallableAbiReceiverInputV1, CanonicalExactLayoutExportsV1,
    CrossConeLayoutAbiSectionV1, ExactCallableAbiError, ExactCallableProtocolV1,
    ExactLayoutExportV1, replay_canonical_scoop_abi_from_layouts,
};
use scoop_wire::{BudgetMeter, WirePath};

use crate::{AbiExpectation, CheckedLayoutLirDependencyV1, CrossConeLayoutLirSemanticClosureError};

#[cfg(test)]
mod tests;

pub(super) fn validate<HE, ME, LE>(
    expectations: &[AbiExpectation],
    local: &CrossConeLayoutAbiSectionV1<'_>,
    dependencies: &[CheckedLayoutLirDependencyV1<'_>],
    meter: &mut BudgetMeter,
) -> Result<(), CrossConeLayoutLirSemanticClosureError<HE, ME, LE>> {
    for expectation in expectations {
        let tables = std::iter::once(local.layouts()).chain(
            dependencies
                .iter()
                .map(|dependency| dependency.layout_abi().layouts()),
        );
        let expected =
            replay(expectation, local.layouts().target(), tables, meter).map_err(|source| {
                CrossConeLayoutLirSemanticClosureError::AbiReplay {
                    provider: expectation.artifact,
                    declaration: expectation.declaration,
                    source: Box::new(source),
                }
            })?;
        expectation.check_canonical(&expected).map_err(|source| {
            CrossConeLayoutLirSemanticClosureError::Relation {
                provider: expectation.artifact,
                source: Box::new(source),
            }
        })?;
    }
    Ok(())
}

fn replay<'a>(
    expectation: &AbiExpectation,
    target: scoop_lir::LirTargetProfile,
    tables: impl Clone + Iterator<Item = &'a CanonicalExactLayoutExportsV1>,
    meter: &mut BudgetMeter,
) -> Result<scoop_identity::CanonicalScoopAbiFunctionSignature, ExactCallableAbiError> {
    let signature = &expectation.signature;
    let receiver = match signature.receiver().into_option() {
        None => CallableAbiReceiverInputV1::NoReceiver,
        Some(exact) => CallableAbiReceiverInputV1::Receiver(find(tables.clone(), exact, meter)?),
    };
    let mut parameters = Vec::new();
    meter.try_reserve_collection_slots(
        &mut parameters,
        signature.parameters().len(),
        &WirePath::root(),
    )?;
    for exact in signature.parameters() {
        parameters.push(find(tables.clone(), *exact, meter)?);
    }
    let result = find(tables, signature.result(), meter)?;
    meter.charge_collection_slots(signature.parameters().len() as u64, &WirePath::root())?;
    meter.charge_owned_bytes(
        (signature.parameters().len() as u64)
            .saturating_mul(std::mem::size_of::<PersistentExactTypeId>() as u64),
        &WirePath::root(),
    )?;
    replay_canonical_scoop_abi_from_layouts(
        target,
        signature.clone(),
        match expectation.gc_effect {
            GcEffect::Managed => ExactCallableProtocolV1::OrdinaryManaged,
            GcEffect::NoGc => ExactCallableProtocolV1::OrdinaryNoGc,
        },
        CallableAbiLayoutInputsV1 {
            receiver,
            parameters: &parameters,
            result,
        },
        meter,
    )
}

fn find<'a>(
    tables: impl Iterator<Item = &'a CanonicalExactLayoutExportsV1>,
    exact: PersistentExactTypeId,
    meter: &mut BudgetMeter,
) -> Result<&'a ExactLayoutExportV1, ExactCallableAbiError> {
    let mut found = None;
    for table in tables {
        meter.charge_work(table.records().len() as u64, &WirePath::root())?;
        if let Some(value) = table.find_exact_role(exact, RepresentationRole::ManagedValue) {
            if found.replace(value).is_some() {
                return Err(ExactCallableAbiError::DuplicateValueLayout { exact });
            }
        }
    }
    found.ok_or(ExactCallableAbiError::MissingValueLayout { exact })
}
