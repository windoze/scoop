//! Cost of the canonical native projection, including private library records.

use super::*;
use scoop_wire::{BudgetMeter, WireEncode, WireError, WireErrorKind, WirePath};

pub(super) fn charge_foundation(
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    table::<CanonicalNativeLibraryRequirementV1>(
        (foundation.native_link_requirements().len() as u64).saturating_mul(3),
        meter,
    )?;
    table::<CanonicalNativeExternalRequirementV1>(
        (foundation.native_contracts().len() as u64).saturating_mul(3),
        meter,
    )?;
    for requirement in foundation.native_link_requirements() {
        copy_plan(requirement.key(), meter)?;
    }
    for record in foundation.native_contracts() {
        copy_plan(record.symbol_key(), meter)?;
        copy_plan(record.contract(), meter)?;
        meter.charge_work(
            (record.symbol_key().native_link_symbol().as_bytes().len() as u64 + 1)
                .saturating_mul(log(foundation.native_contracts().len() as u64)),
            &WirePath::root(),
        )?;
        if let NativeLibraryBinding::Requirement(id) = record.contract().library() {
            meter.charge_work(
                foundation.native_link_requirements().len() as u64,
                &WirePath::root(),
            )?;
            if let Some(requirement) = foundation
                .native_link_requirements()
                .iter()
                .find(|record| record.id() == id)
            {
                copy_plan(requirement.key(), meter)?;
            }
        }
    }
    Ok(())
}

fn table<T>(count: u64, meter: &mut BudgetMeter) -> Result<(), WireError> {
    let path = WirePath::root();
    meter.check_table_entries(count, &path)?;
    meter.charge_collection_slots(count, &path)?;
    meter.charge_owned_bytes(count.saturating_mul(std::mem::size_of::<T>() as u64), &path)?;
    meter.charge_work(count.saturating_mul(log(count)), &path)
}

fn copy_plan(value: &impl WireEncode, meter: &mut BudgetMeter) -> Result<(), WireError> {
    let path = WirePath::root();
    let bytes = scoop_wire::cbor::encoded_length(value)
        .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
    meter.charge_owned_bytes(bytes.saturating_mul(4), &path)?;
    meter.charge_work(bytes.saturating_mul(4), &path)
}

fn log(count: u64) -> u64 {
    1 + u64::from(count.max(1).ilog2())
}
