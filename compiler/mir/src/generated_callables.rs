//! Physical MIR materializations of MIR-generated callable identities.

use std::fmt;

use scoop_identity::{CborIdentityRecord, GeneratedCallableKey, PersistentGeneratedCallableId};

use crate::{CallableSignatureSubject, FunctionId};

pub type GeneratedCallableRecord =
    CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>;

/// One MIR function carrying one callable identity first created by MIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirGeneratedCallableIdentity {
    function: FunctionId,
    identity: GeneratedCallableRecord,
    signature_subject: CallableSignatureSubject,
}

impl MirGeneratedCallableIdentity {
    pub fn new(
        function: FunctionId,
        identity: &GeneratedCallableRecord,
        signature_subject: CallableSignatureSubject,
    ) -> Self {
        Self {
            function,
            identity: identity.clone(),
            signature_subject,
        }
    }

    pub const fn function(&self) -> FunctionId {
        self.function
    }

    pub const fn identity_record(&self) -> &GeneratedCallableRecord {
        &self.identity
    }

    pub const fn signature_subject(&self) -> CallableSignatureSubject {
        self.signature_subject
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
            if let Some((first, _)) = entries[..index]
                .iter()
                .enumerate()
                .find(|(_, existing)| existing.signature_subject == entry.signature_subject)
            {
                return Err(
                    MirGeneratedCallableRelationError::DuplicateSignatureSubject { first, index },
                );
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
    DuplicateSignatureSubject { first: usize, index: usize },
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
            Self::DuplicateSignatureSubject { first, index } => write!(
                formatter,
                "MIR generated callable entries {first} and {index} have the same signature subject"
            ),
        }
    }
}

impl std::error::Error for MirGeneratedCallableRelationError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableOwner, CoreBuiltinNominal, ExactTypeKey, GeneratedCallableKey,
        PersistentExactTypeId,
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

    fn materialization(
        function: u32,
        identity: &GeneratedCallableRecord,
    ) -> MirGeneratedCallableIdentity {
        MirGeneratedCallableIdentity::new(
            FunctionId::from_raw(function.into()),
            identity,
            CallableSignatureSubject::strong(CallableOwner::Generated(identity.id())),
        )
    }

    #[test]
    fn relation_is_sorted_and_queryable_by_both_typed_axes() {
        let first_identity = start(exact(CoreBuiltinNominal::Unit));
        let first = materialization(3, &first_identity);
        let second_identity = start(exact(CoreBuiltinNominal::Any));
        let second = materialization(5, &second_identity);
        let relation =
            MirGeneratedCallableIdentities::checked(vec![first.clone(), second.clone()]).unwrap();
        assert_eq!(
            relation.get(first.function()).unwrap().identity_record(),
            first.identity_record()
        );
        assert_eq!(
            relation.get(first.function()).unwrap().signature_subject(),
            CallableSignatureSubject::strong(CallableOwner::Generated(first_identity.id()))
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
        let first = materialization(0, &identity);
        let other_identity = start(exact(CoreBuiltinNominal::Any));
        let same_function = MirGeneratedCallableIdentity::new(
            first.function(),
            &other_identity,
            CallableSignatureSubject::strong(CallableOwner::Generated(other_identity.id())),
        );
        assert!(matches!(
            MirGeneratedCallableIdentities::checked(vec![first.clone(), same_function]),
            Err(MirGeneratedCallableRelationError::DuplicateFunction { .. })
        ));

        let same_identity = materialization(1, &identity);
        assert!(matches!(
            MirGeneratedCallableIdentities::checked(vec![first.clone(), same_identity]),
            Err(MirGeneratedCallableRelationError::DuplicateIdentity { .. })
        ));

        let same_subject = MirGeneratedCallableIdentity::new(
            FunctionId::from_raw(2_u32.into()),
            &other_identity,
            first.signature_subject(),
        );
        assert!(matches!(
            MirGeneratedCallableIdentities::checked(vec![first, same_subject]),
            Err(MirGeneratedCallableRelationError::DuplicateSignatureSubject { .. })
        ));
    }
}
