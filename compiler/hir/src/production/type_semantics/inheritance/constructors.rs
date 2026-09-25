use super::source_errors::{invalid, resource};
use super::*;
use scoop_wire::WirePath;

pub(super) fn project(
    owner: PersistentExactTypeId,
    inventory: &SourceInheritanceInventoryV1,
    sources: &CanonicalInheritanceSourceConstructorsV1,
) -> Result<CanonicalInheritanceConstructorsV1, Error> {
    let mut records = Vec::new();
    scoop_wire::allocation::try_reserve(
        &mut records,
        inventory.constructors().values().len(),
        &WirePath::root(),
    )
    .map_err(resource)?;
    for declaration in inventory.constructors().values() {
        let source = sources
            .get(*declaration)
            .ok_or(Error::MissingConstructor(owner))?;
        records.push(InheritanceConstructorInterfaceV1::try_new(source.clone()).map_err(invalid)?);
    }
    CanonicalInheritanceConstructorsV1::try_new(records).map_err(invalid)
}
