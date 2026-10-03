//! Each exported descriptor owns exactly its exported physical dispatch tables.

use super::*;
use crate::{ExactDispatchRoleV1, ExactDispatchTableError};
use scoop_identity::{PersistentDispatchTableId, PersistentExactTypeId};

pub(super) fn validate(
    exports: &LayoutAbiExportConstituentsV1,
) -> Result<(), LayoutAbiSectionError> {
    let dispatch = exports.dispatch();
    let mut expected = 0_usize;
    for descriptor in exports.descriptors().records() {
        let tables = descriptor.dispatch();
        expected = expected
            .checked_add(1)
            .and_then(|count| count.checked_add(tables.itables().len()))
            .ok_or(ExactDispatchTableError::CountOverflow)?;
        check(
            dispatch,
            tables.vtable(),
            descriptor.exact(),
            ExactDispatchRoleV1::Vtable,
        )?;
        for table in tables.itables() {
            check(
                dispatch,
                table.table(),
                descriptor.exact(),
                ExactDispatchRoleV1::Itable {
                    interface_exact: table.interface().exact_type(),
                },
            )?;
        }
    }
    if dispatch.records().len() != expected {
        return Err(ExactDispatchTableError::Count {
            expected,
            actual: dispatch.records().len(),
        }
        .into());
    }
    Ok(())
}

fn check(
    dispatch: &crate::CanonicalExactDispatchExportsV1,
    table: PersistentDispatchTableId,
    owner: PersistentExactTypeId,
    role: ExactDispatchRoleV1,
) -> Result<(), LayoutAbiSectionError> {
    let record = dispatch
        .get(table)
        .ok_or(ExactDispatchTableError::Missing(table))?;
    if record.owner_exact() != owner || record.role() != role {
        return Err(LayoutAbiSectionError::DescriptorDispatch(table));
    }
    Ok(())
}
