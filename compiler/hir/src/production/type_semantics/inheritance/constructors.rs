use super::source_resources::{invalid, resource, work};
use super::*;
use scoop_wire::{BudgetMeter, WirePath};

pub(super) fn project(
    owner: PersistentExactTypeId,
    inventory: &SourceInheritanceInventoryV1,
    sources: &CanonicalInheritanceSourceConstructorsV1,
    meter: &mut BudgetMeter,
) -> Result<CanonicalInheritanceConstructorsV1, Error> {
    let mut records = Vec::new();
    meter
        .try_reserve_collection_slots(
            &mut records,
            inventory.constructors().values().len(),
            &WirePath::root(),
        )
        .map_err(resource)?;
    for declaration in inventory.constructors().values() {
        work(meter, sources.records().len())?;
        let source = sources
            .get(*declaration)
            .ok_or(Error::MissingConstructor(owner))?;
        records.push(InheritanceConstructorInterfaceV1::try_new(source.clone()).map_err(invalid)?);
    }
    CanonicalInheritanceConstructorsV1::try_new(records).map_err(invalid)
}
