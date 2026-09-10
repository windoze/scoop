use scoop_identity::{CborIdentityRecord, LocalValueKey, PersistentLocalValueId};

use crate::{FunctionId, LocalId};

pub type SourceLocalValueRecord = CborIdentityRecord<PersistentLocalValueId, LocalValueKey>;

/// One LocalConcrete HIR value after its local slot has crossed into MIR.
///
/// The persistent record remains HIR-owned. MIR retains the exact record and
/// its typed location so later capture and coroutine transforms never recover
/// semantic values from names or arena ordinals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLocalValueIdentity {
    function: FunctionId,
    local: LocalId,
    identity: SourceLocalValueRecord,
}

impl SourceLocalValueIdentity {
    pub const fn new(
        function: FunctionId,
        local: LocalId,
        identity: SourceLocalValueRecord,
    ) -> Self {
        Self {
            function,
            local,
            identity,
        }
    }

    pub const fn function(&self) -> FunctionId {
        self.function
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub const fn identity_record(&self) -> &SourceLocalValueRecord {
        &self.identity
    }
}

/// Complete relation for LocalConcrete HIR values that have MIR local slots.
///
/// Several slots may deliberately name the same semantic value: a lifted
/// local-function capture parameter aliases the value in its lexical owner.
/// Locations, however, are unique.
#[derive(Clone, Debug, Default)]
pub struct SourceLocalValueIdentities {
    entries: Vec<SourceLocalValueIdentity>,
}

impl SourceLocalValueIdentities {
    pub fn checked(
        mut entries: Vec<SourceLocalValueIdentity>,
    ) -> Result<Self, SourceLocalValueRelationError> {
        entries.sort_by_key(SourceLocalValueIdentity::location_sort_key);
        if let Some((first, _)) = entries
            .windows(2)
            .enumerate()
            .find(|(_, pair)| pair[0].location_sort_key() == pair[1].location_sort_key())
        {
            return Err(SourceLocalValueRelationError::DuplicateLocation {
                first,
                index: first + 1,
            });
        }
        Ok(Self { entries })
    }

    pub fn get(&self, function: FunctionId, local: LocalId) -> Option<&SourceLocalValueIdentity> {
        let key = location_sort_key(function, local);
        self.entries
            .binary_search_by_key(&key, SourceLocalValueIdentity::location_sort_key)
            .ok()
            .map(|index| &self.entries[index])
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SourceLocalValueIdentity> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl SourceLocalValueIdentity {
    fn location_sort_key(&self) -> (u32, u32) {
        location_sort_key(self.function, self.local)
    }
}

fn location_sort_key(function: FunctionId, local: LocalId) -> (u32, u32) {
    (function.into_raw().into_u32(), local.into_raw().into_u32())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceLocalValueRelationError {
    DuplicateLocation { first: usize, index: usize },
}

impl std::fmt::Display for SourceLocalValueRelationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateLocation { first, index } => write!(
                formatter,
                "source local value entries {first} and {index} have the same MIR location"
            ),
        }
    }
}

impl std::error::Error for SourceLocalValueRelationError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
        CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
        LocalValueSelector, PackagePath, PersistentFunctionId, SourceDeclarationKey,
        SourceDeclarationSite,
    };

    use super::*;

    fn record() -> SourceLocalValueRecord {
        let declaration = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("sourceLocalOwner").unwrap(),
            0,
            None,
            Vec::new(),
        );
        CborIdentityRecord::from_key(LocalValueKey::new(
            CallableMaterialization::new(
                CallableTemplateOwner::Function(
                    PersistentFunctionId::from_source_declaration(&declaration).unwrap(),
                ),
                CallableMaterializationContext::NoSubstitution,
            ),
            LocalValueSelector::Parameter {
                declaration_index: 0,
            },
        ))
        .unwrap()
    }

    #[test]
    fn captured_aliases_may_share_an_identity_across_typed_locations() {
        let first = SourceLocalValueIdentity::new(
            FunctionId::from_raw(0_u32.into()),
            LocalId::from_raw(0_u32.into()),
            record(),
        );
        let alias = SourceLocalValueIdentity::new(
            FunctionId::from_raw(1_u32.into()),
            LocalId::from_raw(0_u32.into()),
            record(),
        );
        let relation = SourceLocalValueIdentities::checked(vec![first, alias]).unwrap();

        assert_eq!(relation.len(), 2);
        assert_eq!(
            relation
                .get(
                    FunctionId::from_raw(1_u32.into()),
                    LocalId::from_raw(0_u32.into())
                )
                .unwrap()
                .identity_record()
                .id(),
            record().id()
        );
    }

    #[test]
    fn relation_rejects_duplicate_mir_locations() {
        let entry = SourceLocalValueIdentity::new(
            FunctionId::from_raw(0_u32.into()),
            LocalId::from_raw(0_u32.into()),
            record(),
        );
        assert_eq!(
            SourceLocalValueIdentities::checked(vec![entry.clone(), entry]).unwrap_err(),
            SourceLocalValueRelationError::DuplicateLocation { first: 0, index: 1 }
        );
    }
}
