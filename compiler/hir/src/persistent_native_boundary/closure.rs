use std::collections::BTreeSet;

use super::HirNativeBoundaryTypeDefinitionError as Error;
use crate::{NativeBoundaryNominalOwner as Owner, NativeBoundaryTypeDefinitionRecord as Record};

pub(crate) fn close(
    mut required: BTreeSet<Owner>,
    mut definition: impl FnMut(Owner) -> Result<Record, Error>,
) -> Result<Vec<Record>, Error> {
    let mut visited = BTreeSet::new();
    let mut records = Vec::new();
    while let Some(owner) = required.pop_first() {
        if !visited.insert(owner) {
            continue;
        }
        let record = definition(owner)?;
        super::roots::collect_definition_types(&record, &mut required);
        records.push(record);
    }
    records.sort_by(|left, right| left.owner().compare_sort_key(right.owner()));
    Ok(records)
}
