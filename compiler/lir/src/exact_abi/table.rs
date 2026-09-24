use std::sync::Arc;

use scoop_identity::{
    CallableBodyKey, ConeIdentity, PersistentCallableBodyId, StrongCallableDefinitionOwner,
};
use scoop_wire::{BudgetMeter, HashError, WireError, WirePath};

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalExactCallableAbiExportsV1(Arc<CallableAbiTable>);

#[derive(Debug, Eq, PartialEq)]
struct CallableAbiTable {
    provider: ConeIdentity,
    target: LirTargetProfile,
    records: Vec<ExactCallableAbiExportV1>,
}

impl CanonicalExactCallableAbiExportsV1 {
    pub fn try_new(
        target: LirTargetProfile,
        foundation: &OdrFreeLirFoundation,
        mut records: Vec<ExactCallableAbiExportV1>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, ExactCallableAbiTableError> {
        let path = WirePath::root();
        let count = records.len() as u64;
        meter.check_table_entries(count, &path)?;
        meter.charge_collection_slots(count, &path)?;
        let comparisons = count
            .checked_mul(u64::from(count.max(1).ilog2()) + 1)
            .ok_or(ExactCallableAbiTableError::CountOverflow)?;
        meter.charge_work(comparisons, &path)?;
        records.sort_unstable_by_key(ExactCallableAbiExportV1::target);
        for (index, record) in records.iter().enumerate() {
            let callable = record.target();
            if index > 0 && records[index - 1].target() == callable {
                return Err(ExactCallableAbiTableError::Duplicate(callable));
            }
            if record.target_profile() != target {
                return Err(ExactCallableAbiTableError::Target(callable));
            }
            if record.physical_definition().provider() != foundation.producer() {
                return Err(ExactCallableAbiTableError::Provider(callable));
            }
            let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(callable))?;
            meter.charge_work(foundation.callable_bodies().len() as u64, &path)?;
            if !foundation.contains_callable_body(body) {
                return Err(ExactCallableAbiTableError::Body(callable));
            }
            let expected = StrongShapeDefinitionRefV1::from_foundation(
                crate::ExternalStrongShapeSubjectV1::Callable(callable),
                foundation,
                meter,
            )?;
            if expected != record.physical_definition() || record.definition().semantic_id() != body
            {
                return Err(ExactCallableAbiTableError::Definition(callable));
            }
        }
        Ok(Self(Arc::new(CallableAbiTable {
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

    pub fn records(&self) -> &[ExactCallableAbiExportV1] {
        &self.0.records
    }

    pub fn get(&self, target: StrongCallableDefinitionOwner) -> Option<&ExactCallableAbiExportV1> {
        self.0
            .records
            .binary_search_by_key(&target, ExactCallableAbiExportV1::target)
            .ok()
            .map(|index| &self.0.records[index])
    }
}

#[derive(Debug)]
pub enum ExactCallableAbiTableError {
    LayoutProvider,
    LayoutTarget,
    CountOverflow,
    Count,
    Duplicate(StrongCallableDefinitionOwner),
    Target(StrongCallableDefinitionOwner),
    Provider(StrongCallableDefinitionOwner),
    Body(StrongCallableDefinitionOwner),
    Definition(StrongCallableDefinitionOwner),
    PhysicalDefinition(crate::StrongShapeDefinitionError),
    Record {
        index: usize,
        source: ExactCallableAbiWireError,
    },
    Hash(HashError),
    Resource(WireError),
}

impl From<crate::StrongShapeDefinitionError> for ExactCallableAbiTableError {
    fn from(error: crate::StrongShapeDefinitionError) -> Self {
        Self::PhysicalDefinition(error)
    }
}

impl From<HashError> for ExactCallableAbiTableError {
    fn from(error: HashError) -> Self {
        Self::Hash(error)
    }
}

impl From<WireError> for ExactCallableAbiTableError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for ExactCallableAbiTableError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid exact callable ABI table: {self:?}")
    }
}

impl std::error::Error for ExactCallableAbiTableError {}
