//! Physical MIR materializations of MIR-generated callable identities.

use std::fmt;

use scoop_identity::{CborIdentityRecord, GeneratedCallableKey, PersistentGeneratedCallableId};

use crate::FunctionId;

pub type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;

/// One MIR function carrying one callable identity first created by MIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirGeneratedCallableIdentity {
    function: FunctionId,
    identity: GeneratedCallableRecord,
}

impl MirGeneratedCallableIdentity {
    pub fn new(function: FunctionId, identity: &GeneratedCallableRecord) -> Self {
        Self {
            function,
            identity: identity.clone(),
        }
    }

    pub const fn function(&self) -> FunctionId {
        self.function
    }

    pub const fn identity_record(&self) -> &GeneratedCallableRecord {
        &self.identity
    }
}

/// Complete one-to-one relation for callable identities first created by MIR.
#[derive(Clone, Debug, Default)]
pub struct MirGeneratedCallableIdentities {
    entries: Vec<MirGeneratedCallableIdentity>,
}

impl MirGeneratedCallableIdentities {
    pub fn checked(
        mut entries: Vec<MirGeneratedCallableIdentity>,
    ) -> Result<Self, MirGeneratedCallableRelationError> {
        entries.sort_by_key(|entry| entry.identity.id());
        for (index, entry) in entries.iter().enumerate() {
            if let Some((first, _)) = entries[..index]
                .iter()
                .enumerate()
                .find(|(_, existing)| existing.function == entry.function)
            {
                return Err(MirGeneratedCallableRelationError::DuplicateFunction { first, index });
            }
            if let Some((first, _)) = entries[..index]
                .iter()
                .enumerate()
                .find(|(_, existing)| existing.identity.id() == entry.identity.id())
            {
                return Err(MirGeneratedCallableRelationError::DuplicateIdentity { first, index });
            }
        }
        Ok(Self { entries })
    }

    pub fn get(&self, function: FunctionId) -> Option<&MirGeneratedCallableIdentity> {
        self.entries
            .iter()
            .find(|entry| entry.function() == function)
    }

    pub fn get_by_identity(
        &self,
        identity: PersistentGeneratedCallableId,
    ) -> Option<&MirGeneratedCallableIdentity> {
        self.entries
            .iter()
            .find(|entry| entry.identity_record().id() == identity)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirGeneratedCallableIdentity> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirGeneratedCallableRelationError {
    DuplicateFunction { first: usize, index: usize },
    DuplicateIdentity { first: usize, index: usize },
}

impl fmt::Display for MirGeneratedCallableRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateFunction { first, index } => write!(
                formatter,
                "MIR generated callable entries {first} and {index} name the same function"
            ),
            Self::DuplicateIdentity { first, index } => write!(
                formatter,
                "MIR generated callable entries {first} and {index} have the same identity"
            ),
        }
    }
}

impl std::error::Error for MirGeneratedCallableRelationError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CoreBuiltinNominal, ExactTypeKey, GeneratedCallableKey, PersistentExactTypeId,
    };

    use super::*;

    fn exact(nominal: CoreBuiltinNominal) -> PersistentExactTypeId {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal.identity_record().id()))
            .unwrap()
            .id()
    }

    fn start(result: PersistentExactTypeId) -> GeneratedCallableRecord {
        CborIdentityRecord::from_key(GeneratedCallableKey::CoroutineStart { result }).unwrap()
    }

    #[test]
    fn relation_is_sorted_and_queryable_by_both_typed_axes() {
        let first = MirGeneratedCallableIdentity::new(
            FunctionId::from_raw(3_u32.into()),
            &start(exact(CoreBuiltinNominal::Unit)),
        );
        let second = MirGeneratedCallableIdentity::new(
            FunctionId::from_raw(5_u32.into()),
            &start(exact(CoreBuiltinNominal::Any)),
        );
        let relation =
            MirGeneratedCallableIdentities::checked(vec![first.clone(), second.clone()]).unwrap();
        assert_eq!(
            relation.get(first.function()).unwrap().identity_record(),
            first.identity_record()
        );
        assert_eq!(
            relation
                .get_by_identity(second.identity_record().id())
                .unwrap()
                .function(),
            second.function()
        );
        assert!(
            relation
                .iter()
                .map(|entry| entry.identity_record().id())
                .is_sorted()
        );
    }

    #[test]
    fn relation_rejects_duplicate_functions_and_identities() {
        let identity = start(exact(CoreBuiltinNominal::Unit));
        let first =
            MirGeneratedCallableIdentity::new(FunctionId::from_raw(0_u32.into()), &identity);
        let same_function = MirGeneratedCallableIdentity::new(
            first.function(),
            &start(exact(CoreBuiltinNominal::Any)),
        );
        assert!(matches!(
            MirGeneratedCallableIdentities::checked(vec![first.clone(), same_function]),
            Err(MirGeneratedCallableRelationError::DuplicateFunction { .. })
        ));

        let same_identity =
            MirGeneratedCallableIdentity::new(FunctionId::from_raw(1_u32.into()), &identity);
        assert!(matches!(
            MirGeneratedCallableIdentities::checked(vec![first, same_identity]),
            Err(MirGeneratedCallableRelationError::DuplicateIdentity { .. })
        ));
    }
}
