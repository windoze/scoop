use super::*;
use std::collections::BTreeMap;

pub(super) fn copy_native(
    native: &lir::CanonicalNativeExternalRequirementSurfaceV1,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    slots::<lir::CanonicalNativeExternalRequirementV1>(native.contracts().len() as u64, meter)?;
    slots::<lir::CanonicalNativeLibraryRequirementV1>(
        native.library_requirements().len() as u64,
        meter,
    )?;
    for record in native.library_requirements() {
        copy_plan(record.key(), meter)?;
    }
    for record in native.contracts() {
        copy_requirement(record, meter)?;
    }
    Ok(())
}

fn copy_requirement(
    record: &lir::CanonicalNativeExternalRequirementV1,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    copy_plan(record.symbol_key(), meter)?;
    copy_plan(record.contract(), meter)?;
    slots::<scoop_identity::PersistentSourceNativeExternalContractId>(
        record.sources().len() as u64,
        meter,
    )?;
    if let Some(requirement) = record.library().requirement() {
        copy_plan(requirement.key(), meter)?;
    }
    Ok(())
}

pub(super) fn source_copies(
    shape: &VerifiedExternalShapeRequirementClosureV1<'_>,
    native: &lir::CanonicalNativeExternalRequirementSurfaceV1,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let path = WirePath::root();
    let count = native.contracts().len() as u64;
    table::<(&[u8], &lir::CanonicalNativeExternalRequirementV1)>(count, meter)?;
    let mut requirements = BTreeMap::new();
    for record in native.contracts() {
        let name = record.symbol_key().native_link_symbol().as_bytes();
        meter.charge_owned_bytes(name.len() as u64, &path)?;
        meter.charge_work((name.len() as u64 + 1).saturating_mul(log(count)), &path)?;
        requirements.insert(name, record);
    }
    for binding in shape.remaining_external_candidates() {
        meter.charge_work(
            (binding.symbol().len() as u64 + 1).saturating_mul(log(count)),
            &path,
        )?;
        if let Some(requirement) = requirements.get(binding.symbol()) {
            copy_requirement(requirement, meter)?;
        }
    }
    Ok(())
}
