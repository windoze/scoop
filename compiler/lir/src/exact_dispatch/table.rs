use std::sync::Arc;

use scoop_identity::{ConeIdentity, PersistentDispatchTableId};

use super::*;
use crate::{
    ConeLirFoundation, ExternalStrongShapeSubjectV1, LirTargetProfile, StrongShapeDefinitionRefV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExactDispatchExportsV1(Arc<ExactDispatchTableBodyV1>);

#[derive(Debug, Eq, PartialEq)]
struct ExactDispatchTableBodyV1 {
    provider: ConeIdentity,
    target: LirTargetProfile,
    records: Vec<ExactDispatchExportV1>,
}

impl CanonicalExactDispatchExportsV1 {
    pub fn try_new(
        target: LirTargetProfile,
        foundation: &ConeLirFoundation,
        mut records: Vec<ExactDispatchExportV1>,
    ) -> Result<Self, ExactDispatchTableError> {
        records.sort_unstable_by_key(ExactDispatchExportV1::table);
        for pair in records.windows(2) {
            if pair[0].table() == pair[1].table() {
                return Err(ExactDispatchTableError::Duplicate(pair[0].table()));
            }
        }
        validate_layouts(&records)?;
        let expected = foundation.dispatch_tables();
        for record in &records {
            expected
                .binary_search_by_key(&record.table(), |identity| identity.id())
                .map_err(|_| ExactDispatchTableError::Missing(record.table()))?;
            if record.physical_definition().provider() != foundation.producer() {
                return Err(ExactDispatchTableError::Provider(record.table()));
            }
            if record.target() != target {
                return Err(ExactDispatchTableError::Target(record.table()));
            }
            let physical = StrongShapeDefinitionRefV1::from_foundation(
                ExternalStrongShapeSubjectV1::DispatchTable(record.table()),
                foundation,
            )?;
            if record.physical_definition() != physical
                || record.definition().semantic_id() != record.table()
                || record.definition().definition_plan() != physical.definition()
                || record.definition().symbol() != physical.symbol()
            {
                return Err(ExactDispatchTableError::Definition(record.table()));
            }
        }
        Ok(Self(Arc::new(ExactDispatchTableBodyV1 {
            provider: foundation.producer(),
            target,
            records,
        })))
    }

    pub(super) fn from_saved(
        target: LirTargetProfile,
        provider: ConeIdentity,
        records: Vec<ExactDispatchExportV1>,
    ) -> Self {
        Self(Arc::new(ExactDispatchTableBodyV1 {
            provider,
            target,
            records,
        }))
    }

    pub fn provider(&self) -> ConeIdentity {
        self.0.provider
    }

    pub fn target(&self) -> LirTargetProfile {
        self.0.target
    }

    pub fn records(&self) -> &[ExactDispatchExportV1] {
        &self.0.records
    }

    pub fn get(&self, table: PersistentDispatchTableId) -> Option<&ExactDispatchExportV1> {
        self.0
            .records
            .binary_search_by_key(&table, ExactDispatchExportV1::table)
            .ok()
            .map(|index| &self.0.records[index])
    }
}

pub(super) fn validate_layouts(
    records: &[ExactDispatchExportV1],
) -> Result<(), ExactDispatchTableError> {
    let find = |id| {
        records
            .binary_search_by_key(&id, ExactDispatchExportV1::table)
            .ok()
            .map(|index| &records[index])
    };
    for table in records {
        let invalid = || ExactDispatchTableError::Layout(table.table());
        let layout = table.layout();
        let storage = find(layout.storage()).ok_or_else(invalid)?;
        if storage.owner_exact() != table.owner_exact()
            || storage.layout().storage() != storage.table()
            || storage.layout().offset() != 0
        {
            return Err(invalid());
        }
        let start = layout.offset() as usize;
        let end = start
            .checked_add(layout.slots().len())
            .ok_or_else(invalid)?;
        if storage.layout().slots().get(start..end) != Some(layout.slots()) {
            return Err(invalid());
        }
        if matches!(table.role(), ExactDispatchRoleV1::Vtable)
            && layout != &crate::DispatchTableLayout::flat(table.table(), table.entries().len())
        {
            return Err(invalid());
        }
        for source in layout.slots() {
            let origin = find(source.table).ok_or_else(invalid)?;
            if origin.owner_exact() != table.owner_exact()
                || origin.entries().get(source.method as usize).is_none()
                || matches!(origin.role(), ExactDispatchRoleV1::Vtable)
                    != matches!(table.role(), ExactDispatchRoleV1::Vtable)
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}
