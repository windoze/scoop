use scoop_identity::{CborIdentityRecord, LocalValueKey, PersistentLocalValueId};

use crate::{FunctionId, LocalId};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum LocalValueOwner {
    Function(FunctionId),
    ReleaseHook(crate::ReleaseHookId),
}

impl From<FunctionId> for LocalValueOwner {
    fn from(function: FunctionId) -> Self {
        Self::Function(function)
    }
}

impl From<crate::ReleaseHookId> for LocalValueOwner {
    fn from(hook: crate::ReleaseHookId) -> Self {
        Self::ReleaseHook(hook)
    }
}

pub type LocalValueIdentityRecord = CborIdentityRecord<PersistentLocalValueId, LocalValueKey>;

/// The IR stage that first created a persistent local-value identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalValueIdentityAuthority {
    Hir,
    Mir,
}

/// One persistent semantic value attached to its typed MIR local slot.
///
/// The value may be transposed from LocalConcrete HIR or introduced during
/// MIR CFG construction. Later transforms retain this exact relation instead
/// of recovering semantic values from display names or arena ordinals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalValueIdentity {
    owner: LocalValueOwner,
    local: LocalId,
    authority: LocalValueIdentityAuthority,
    identity: LocalValueIdentityRecord,
}

impl LocalValueIdentity {
    pub const fn from_hir(
        function: FunctionId,
        local: LocalId,
        identity: LocalValueIdentityRecord,
    ) -> Self {
        Self {
            owner: LocalValueOwner::Function(function),
            local,
            authority: LocalValueIdentityAuthority::Hir,
            identity,
        }
    }

    pub const fn from_mir(
        function: FunctionId,
        local: LocalId,
        identity: LocalValueIdentityRecord,
    ) -> Self {
        Self {
            owner: LocalValueOwner::Function(function),
            local,
            authority: LocalValueIdentityAuthority::Mir,
            identity,
        }
    }

    pub const fn owner(&self) -> LocalValueOwner {
        self.owner
    }

    pub const fn for_owner(
        owner: LocalValueOwner,
        local: LocalId,
        authority: LocalValueIdentityAuthority,
        identity: LocalValueIdentityRecord,
    ) -> Self {
        Self {
            owner,
            local,
            authority,
            identity,
        }
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub const fn authority(&self) -> LocalValueIdentityAuthority {
        self.authority
    }

    pub const fn identity_record(&self) -> &LocalValueIdentityRecord {
        &self.identity
    }

    pub fn relocated(&self, function: FunctionId, local: LocalId) -> Self {
        Self {
            owner: LocalValueOwner::Function(function),
            local,
            authority: self.authority,
            identity: self.identity.clone(),
        }
    }
}

/// Complete relation for persistent semantic values that have MIR local slots.
///
/// Several slots may deliberately name the same semantic value: a lifted
/// local-function capture parameter aliases the value in its lexical owner.
/// Locations, however, are unique.
#[derive(Clone, Debug, Default)]
pub struct LocalValueIdentities {
    entries: Vec<LocalValueIdentity>,
}

impl LocalValueIdentities {
    pub fn checked(
        mut entries: Vec<LocalValueIdentity>,
    ) -> Result<Self, LocalValueIdentityRelationError> {
        entries.sort_by_key(LocalValueIdentity::location_sort_key);
        if let Some((first, _)) = entries
            .windows(2)
            .enumerate()
            .find(|(_, pair)| pair[0].location_sort_key() == pair[1].location_sort_key())
        {
            return Err(LocalValueIdentityRelationError::DuplicateLocation {
                first,
                index: first + 1,
            });
        }
        Ok(Self { entries })
    }

    pub fn get(&self, function: FunctionId, local: LocalId) -> Option<&LocalValueIdentity> {
        self.get_owned(LocalValueOwner::Function(function), local)
    }

    pub fn get_owned(&self, owner: LocalValueOwner, local: LocalId) -> Option<&LocalValueIdentity> {
        let key = location_sort_key(owner, local);
        self.entries
            .binary_search_by_key(&key, LocalValueIdentity::location_sort_key)
            .ok()
            .map(|index| &self.entries[index])
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &LocalValueIdentity> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl LocalValueIdentity {
    fn location_sort_key(&self) -> (u8, u32, u32) {
        location_sort_key(self.owner, self.local)
    }
}

fn location_sort_key(owner: LocalValueOwner, local: LocalId) -> (u8, u32, u32) {
    let (kind, index) = match owner {
        LocalValueOwner::Function(function) => (0, function.into_raw().into_u32()),
        LocalValueOwner::ReleaseHook(hook) => (1, hook.into_raw().into_u32()),
    };
    (kind, index, local.into_raw().into_u32())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalValueIdentityRelationError {
    DuplicateLocation { first: usize, index: usize },
}

impl std::fmt::Display for LocalValueIdentityRelationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateLocation { first, index } => write!(
                formatter,
                "local value entries {first} and {index} have the same MIR location"
            ),
        }
    }
}

impl std::error::Error for LocalValueIdentityRelationError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
        CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
        LocalValueSelector, PackagePath, PersistentFunctionId, SourceDeclarationKey,
        SourceDeclarationSite,
    };

    use super::*;

    fn record() -> LocalValueIdentityRecord {
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
        let first = LocalValueIdentity::from_hir(
            FunctionId::from_raw(0_u32.into()),
            LocalId::from_raw(0_u32.into()),
            record(),
        );
        let alias = LocalValueIdentity::from_hir(
            FunctionId::from_raw(1_u32.into()),
            LocalId::from_raw(0_u32.into()),
            record(),
        );
        let relation = LocalValueIdentities::checked(vec![first, alias]).unwrap();

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
        let entry = LocalValueIdentity::from_hir(
            FunctionId::from_raw(0_u32.into()),
            LocalId::from_raw(0_u32.into()),
            record(),
        );
        assert_eq!(
            LocalValueIdentities::checked(vec![entry.clone(), entry]).unwrap_err(),
            LocalValueIdentityRelationError::DuplicateLocation { first: 0, index: 1 }
        );
    }
}
