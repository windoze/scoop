use std::sync::Arc;

use scoop_identity::{ConeIdentity, PersistentDispatchTableId};

use super::*;
use crate::{
    ExternalStrongShapeSubjectV1, LirTargetProfile, OdrFreeLirFoundation,
    StrongShapeDefinitionRefV1,
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
        foundation: &OdrFreeLirFoundation,
        mut records: Vec<ExactDispatchExportV1>,
    ) -> Result<Self, ExactDispatchTableError> {
        records.sort_unstable_by_key(ExactDispatchExportV1::table);
        for pair in records.windows(2) {
            if pair[0].table() == pair[1].table() {
                return Err(ExactDispatchTableError::Duplicate(pair[0].table()));
            }
        }
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
