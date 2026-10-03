//! Complete managed signatures of callable implementations present in MIR.

use std::fmt;

use crate::{CallableSignatureRecord, CallableSignatureSubject};

/// Canonically ordered, one-to-one relation from implementation subjects to
/// their complete Scoop signatures.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirCallableSignatures {
    entries: Vec<CallableSignatureRecord>,
}

impl MirCallableSignatures {
    pub fn checked(
        mut entries: Vec<CallableSignatureRecord>,
    ) -> Result<Self, MirCallableSignatureRelationError> {
        entries.sort_by(|left, right| left.subject().compare_sort_key(right.subject()));
        if let Some((first, pair)) = entries.windows(2).enumerate().find(|(_, pair)| {
            pair[0]
                .subject()
                .compare_sort_key(pair[1].subject())
                .is_eq()
        }) {
            return Err(MirCallableSignatureRelationError::DuplicateSubject {
                first,
                index: first + 1,
                subject: pair[0].subject(),
            });
        }
        Ok(Self { entries })
    }

    pub fn get(&self, subject: CallableSignatureSubject) -> Option<&CallableSignatureRecord> {
        self.entries
            .binary_search_by(|entry| entry.subject().compare_sort_key(subject))
            .ok()
            .map(|index| &self.entries[index])
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &CallableSignatureRecord> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn into_records(self) -> Vec<CallableSignatureRecord> {
        self.entries
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirCallableSignatureRelationError {
    DuplicateSubject {
        first: usize,
        index: usize,
        subject: CallableSignatureSubject,
    },
}

impl fmt::Display for MirCallableSignatureRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateSubject {
                first,
                index,
                subject,
            } => write!(
                formatter,
                "MIR callable signature entries {first} and {index} have the same subject {}:{:02x?}",
                subject.kind_tag(),
                subject.raw_id()
            ),
        }
    }
}

impl std::error::Error for MirCallableSignatureRelationError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableOwner, CborIdentityRecord, CoreBuiltinNominal, Effect, ExactCallableSignature,
        ExactTypeKey, GeneratedCallableKey,
    };

    use super::*;

    fn exact(nominal: CoreBuiltinNominal) -> scoop_identity::PersistentExactTypeId {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal.identity_record().id()))
            .unwrap()
            .id()
    }

    fn record(result: CoreBuiltinNominal) -> CallableSignatureRecord {
        let result = exact(result);
        let callable =
            CborIdentityRecord::from_key(GeneratedCallableKey::CoroutineStart { result }).unwrap();
        CallableSignatureRecord::new(
            CallableSignatureSubject::strong(CallableOwner::Generated(callable.id())),
            ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), result),
        )
    }

    #[test]
    fn relation_is_sorted_and_queryable_by_subject() {
        let first = record(CoreBuiltinNominal::Unit);
        let second = record(CoreBuiltinNominal::Any);
        let relation = MirCallableSignatures::checked(vec![first.clone(), second.clone()]).unwrap();

        assert_eq!(
            relation.get(first.subject()).unwrap().signature(),
            first.signature()
        );
        assert!(
            relation
                .iter()
                .map(CallableSignatureRecord::subject)
                .is_sorted_by(|left, right| left.compare_sort_key(*right).is_le())
        );
    }

    #[test]
    fn relation_rejects_duplicate_subjects() {
        let first = record(CoreBuiltinNominal::Unit);
        let second = CallableSignatureRecord::new(
            first.subject(),
            ExactCallableSignature::new(
                Effect::Suspend,
                None,
                Vec::new(),
                exact(CoreBuiltinNominal::Any),
            ),
        );

        assert!(matches!(
            MirCallableSignatures::checked(vec![first, second]),
            Err(MirCallableSignatureRelationError::DuplicateSubject { .. })
        ));
    }
}
