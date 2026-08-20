use std::collections::HashMap;

use scoop_identity::{StableIdentityOrderError, stable_topological_identity_order};

use super::*;

pub type CallbackApplicationRecord =
    CborIdentityRecord<PersistentCallbackApplicationId, CallbackApplicationKey>;

/// Complete canonical callback-application table referenced by LocalConcrete
/// HIR. Callback applications cannot depend on one another, so their stable
/// order is their persistent identity order.
#[derive(Clone, Debug)]
pub struct CallbackApplicationIdentities {
    records: Vec<CallbackApplicationRecord>,
    positions: HashMap<PersistentCallbackApplicationId, usize>,
}

impl CallbackApplicationIdentities {
    pub fn checked(
        records: Vec<CallbackApplicationRecord>,
    ) -> Result<Self, StableIdentityOrderError<PersistentCallbackApplicationId>> {
        let records =
            stable_topological_identity_order(records, CborIdentityRecord::id, |_| Vec::new())?;
        let positions = records
            .iter()
            .enumerate()
            .map(|(position, record)| (record.id(), position))
            .collect();
        Ok(Self { records, positions })
    }

    pub fn get(&self, id: PersistentCallbackApplicationId) -> Option<&CallbackApplicationRecord> {
        self.positions
            .get(&id)
            .map(|position| &self.records[*position])
    }

    pub fn records(&self) -> &[CallbackApplicationRecord] {
        &self.records
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
