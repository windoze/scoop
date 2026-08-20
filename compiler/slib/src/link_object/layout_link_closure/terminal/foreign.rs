//! Check both arrival orders without granting sibling dependency access.

use super::*;

pub(crate) fn reject_layout_foreign_strong_owners_v1<'a>(
    consumer: ConeIdentity,
    target: scoop_lir::LirTargetProfile,
    imports: &scoop_lir::CanonicalExternalShapeLinkImportsV1,
    owners: impl Iterator<Item = &'a CanonicalDefinedLinkSymbolOwnerSetV1> + Clone,
) -> Result<(), CrossConeLayoutTerminalValidationError> {
    for import in imports.records() {
        let symbol = import.expected_symbol();
        let name = owner::normalized_symbol(target, symbol);
        owner::reject_foreign_owners(consumer, import, owners.clone(), &name, symbol)?;
    }
    Ok(())
}
