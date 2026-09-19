use super::*;
use scoop_wire::{BudgetMeter, WirePath};

impl DecodedDeclarationAccessSourceV1 {
    /// Accounts for lexical owners, their output/uniqueness storage, and the
    /// complete inline origin before invoking any identity resolver.
    pub fn charge_resolution_at(
        &self,
        meter: &mut BudgetMeter,
        path: &WirePath,
        depth: u64,
    ) -> Result<(), WireError> {
        meter.check_semantic_depth(depth, path)?;
        meter.charge_nodes(1, path)?;
        meter.charge_edges(3, path)?;
        meter.charge_work(1, path)?;
        let at = path.clone().field(2);
        let count = self.lexical_owners.len() as u64;
        meter.check_table_entries(count, &at)?;
        let work = count
            .checked_mul(33)
            .ok_or_else(|| WireError::new(WireErrorKind::IntegerOutOfRange, at.clone(), None))?;
        meter.charge_work(work, &at)?;
        meter.charge_collection_slots(count, &at)?;
        meter.charge_collection_slots(count, &at)?;
        meter.charge_nodes(count, &at)?;
        meter.charge_edges(count, &at)?;
        let next = depth
            .checked_add(1)
            .ok_or_else(|| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        self.definition_origin
            .charge_resolution_at(meter, &path.clone().field(3), next)
    }
}
