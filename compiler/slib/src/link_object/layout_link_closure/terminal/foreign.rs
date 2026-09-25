//! Check both arrival orders without granting sibling dependency access.

use super::*;

pub(crate) fn reject_layout_foreign_strong_owners_v1<'a>(
    consumer: ConeIdentity,
    target: scoop_lir::LirTargetProfile,
    imports: &scoop_lir::CanonicalExternalShapeLinkImportsV1<'_>,
    owners: impl Iterator<Item = &'a CanonicalDefinedLinkSymbolOwnerSetV1> + Clone,
    meter: &mut BudgetMeter,
) -> Result<(), CrossConeLayoutTerminalValidationError> {
    let path = WirePath::root();
    meter.charge_work((imports.records().len() as u64).saturating_add(1), &path)?;
    for import in imports.records() {
        let symbol = import.expected_symbol();
        let name = owner::normalized_symbol(target, symbol, meter)?;
        owner::reject_foreign_owners(consumer, import, owners.clone(), &name, symbol, meter)?;
    }
    Ok(())
}
