//! Each exported descriptor owns exactly its exported physical dispatch tables.

use super::*;
use crate::{ExactDispatchRoleV1, ExactDispatchTableError};
use scoop_identity::{PersistentDispatchTableId, PersistentExactTypeId};

pub(super) fn validate<E>(
    exports: &LayoutAbiExportConstituentsV1,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSectionError<E>> {
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
            meter,
        )?;
        for table in tables.itables() {
            check(
                dispatch,
                table.table(),
                descriptor.exact(),
                ExactDispatchRoleV1::Itable {
                    interface_exact: table.interface().exact_type(),
                },
                meter,
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

fn check<E>(
    dispatch: &crate::CanonicalExactDispatchExportsV1,
    table: PersistentDispatchTableId,
    owner: PersistentExactTypeId,
    role: ExactDispatchRoleV1,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSectionError<E>> {
    meter.charge_work(
        u64::from(dispatch.records().len().max(1).ilog2()) + 1,
        &WirePath::root(),
    )?;
    let record = dispatch
        .get(table)
        .ok_or(ExactDispatchTableError::Missing(table))?;
    if record.owner_exact() != owner || record.role() != role {
        return Err(LayoutAbiSectionError::DescriptorDispatch(table));
    }
    Ok(())
}
