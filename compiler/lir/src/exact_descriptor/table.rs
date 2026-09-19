use std::sync::Arc;

use scoop_identity::{ConeIdentity, PersistentExactTypeId};
use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::*;
use crate::{LirTargetProfile, OdrFreeLirFoundation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExactDescriptorExportsV1(Arc<DescriptorTable>);

#[derive(Debug, Eq, PartialEq)]
struct DescriptorTable {
    provider: ConeIdentity,
    target: LirTargetProfile,
    records: Vec<ExactDescriptorExportV1>,
}

impl CanonicalExactDescriptorExportsV1 {
    pub fn try_new(
        target: LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        mut records: Vec<ExactDescriptorExportV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactDescriptorTableError> {
        let path = WirePath::root();
        let count = records.len() as u64;
        meter.check_table_entries(count, &path)?;
        meter.charge_collection_slots(count, &path)?;
        let comparisons = count
            .checked_mul(u64::from(count.max(1).ilog2()) + 1)
            .ok_or(ExactDescriptorTableError::CountOverflow)?;
        meter.charge_work(comparisons, &path)?;
        records.sort_unstable_by_key(ExactDescriptorExportV1::exact);
        for (index, record) in records.iter().enumerate() {
            if index > 0 && records[index - 1].exact() == record.exact() {
                return Err(ExactDescriptorTableError::Duplicate(record.exact()));
            }
            if record.value_layout().identity().target() != target
                || record.instance_layout().identity().target() != target
            {
                return Err(ExactDescriptorTableError::Target(record.exact()));
            }
            if record.physical_definition().provider() != foundation.producer() {
                return Err(ExactDescriptorTableError::Provider(record.exact()));
            }
            let expected = crate::StrongShapeDefinitionRefV1::from_foundation(
                crate::ExternalStrongShapeSubjectV1::TypeDescriptor(record.exact()),
                foundation,
                meter,
            )?;
            if expected != record.physical_definition() {
                return Err(ExactDescriptorTableError::Definition(record.exact()));
            }
        }
        Ok(Self(Arc::new(DescriptorTable {
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

    pub fn records(&self) -> &[ExactDescriptorExportV1] {
        &self.0.records
    }

    pub fn get(&self, exact: PersistentExactTypeId) -> Option<&ExactDescriptorExportV1> {
        self.0
            .records
            .binary_search_by_key(&exact, ExactDescriptorExportV1::exact)
            .ok()
            .map(|index| &self.0.records[index])
    }
}

#[derive(Debug)]
pub enum ExactDescriptorTableError {
    CountOverflow,
    Count,
    Duplicate(PersistentExactTypeId),
    Target(PersistentExactTypeId),
    Provider(PersistentExactTypeId),
    Definition(PersistentExactTypeId),
    PhysicalDefinition(crate::StrongShapeDefinitionError),
    Record {
        index: usize,
        source: ExactDescriptorWireError,
    },
    Resource(WireError),
}

impl From<crate::StrongShapeDefinitionError> for ExactDescriptorTableError {
    fn from(error: crate::StrongShapeDefinitionError) -> Self {
        Self::PhysicalDefinition(error)
    }
}

impl From<WireError> for ExactDescriptorTableError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for ExactDescriptorTableError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid exact TypeDescriptor table: {self:?}")
    }
}

impl std::error::Error for ExactDescriptorTableError {}
