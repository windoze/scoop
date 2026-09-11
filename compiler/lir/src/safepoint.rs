use std::collections::BTreeMap;
use std::fmt;
use std::ops::Index;

pub use scoop_identity::{PersistentSafepointSiteId, SafepointId, SafepointSiteRole};

pub type SafepointSiteIdentityRecord =
    scoop_identity::CborIdentityRecord<PersistentSafepointSiteId, scoop_identity::SafepointSiteKey>;

/// Complete persistent and runtime identity of one final LIR safepoint site.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafepointIdentity {
    site: SafepointSiteIdentityRecord,
    runtime: SafepointId,
}

impl SafepointIdentity {
    pub fn new(
        owner: scoop_identity::PersistentCallableBodyId,
        role: SafepointSiteRole,
        ordinal: u32,
    ) -> Result<Self, SafepointIdentityBuildError> {
        let site = scoop_identity::CborIdentityRecord::from_key(
            scoop_identity::SafepointSiteKey::new(owner, role, ordinal),
        )
        .map_err(SafepointIdentityBuildError::Site)?;
        let runtime =
            SafepointId::derive(site.id()).map_err(SafepointIdentityBuildError::Runtime)?;
        Ok(Self { site, runtime })
    }

    pub const fn site_record(&self) -> &SafepointSiteIdentityRecord {
        &self.site
    }

    pub const fn site_id(&self) -> PersistentSafepointSiteId {
        self.site.id()
    }

    pub const fn runtime_id(&self) -> SafepointId {
        self.runtime
    }

    pub const fn owner(&self) -> scoop_identity::PersistentCallableBodyId {
        self.site.key().owner()
    }

    pub const fn role(&self) -> SafepointSiteRole {
        self.site.key().role()
    }

    pub const fn ordinal(&self) -> u32 {
        self.site.key().ordinal()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SafepointIdentityBuildError {
    Site(scoop_wire::HashError),
    Runtime(scoop_identity::DerivedIdError),
}

impl fmt::Display for SafepointIdentityBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Site(error) => error.fmt(formatter),
            Self::Runtime(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SafepointIdentityBuildError {}

/// Complete safepoint identity relation for one function's final CFG.
#[derive(Debug, Default)]
pub struct SafepointIdentities {
    entries: BTreeMap<super::SafepointSiteRef, SafepointIdentity>,
}

impl SafepointIdentities {
    pub fn checked(
        entries: Vec<(super::SafepointSiteRef, SafepointIdentity)>,
    ) -> Result<Self, SafepointIdentityRelationError> {
        let mut relation = BTreeMap::new();
        let mut sites = BTreeMap::new();
        let mut runtime_ids = BTreeMap::new();
        for (index, (reference, entry)) in entries.into_iter().enumerate() {
            if let Some((first, _)) = relation.get(&reference) {
                return Err(SafepointIdentityRelationError::DuplicateReference {
                    first: *first,
                    index,
                });
            }
            if let Some(first) = sites.insert(entry.site_id(), index) {
                return Err(SafepointIdentityRelationError::DuplicateSite { first, index });
            }
            if let Some(first) = runtime_ids.insert(entry.runtime_id(), index) {
                return Err(SafepointIdentityRelationError::DuplicateRuntimeId { first, index });
            }
            relation.insert(reference, (index, entry));
        }
        Ok(Self {
            entries: relation
                .into_iter()
                .map(|(reference, (_, identity))| (reference, identity))
                .collect(),
        })
    }

    pub fn get(&self, reference: super::SafepointSiteRef) -> Option<&SafepointIdentity> {
        self.entries.get(&reference)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SafepointIdentity> {
        self.entries.values()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Index<super::SafepointSiteRef> for SafepointIdentities {
    type Output = SafepointIdentity;

    fn index(&self, index: super::SafepointSiteRef) -> &Self::Output {
        &self.entries[&index]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SafepointIdentityRelationError {
    DuplicateReference { first: usize, index: usize },
    DuplicateSite { first: usize, index: usize },
    DuplicateRuntimeId { first: usize, index: usize },
}

impl fmt::Display for SafepointIdentityRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateReference { first, index } => write!(
                formatter,
                "safepoint entries {first} and {index} have the same function-local reference"
            ),
            Self::DuplicateSite { first, index } => write!(
                formatter,
                "safepoint entries {first} and {index} have the same persistent site identity"
            ),
            Self::DuplicateRuntimeId { first, index } => write!(
                formatter,
                "safepoint entries {first} and {index} have the same runtime id"
            ),
        }
    }
}

impl std::error::Error for SafepointIdentityRelationError {}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CallableBodyKey, CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
        PackagePath, PersistentCallableBodyId, PersistentFunctionId, SourceDeclarationKey,
        SourceDeclarationSite, StrongCallableDefinitionOwner,
    };

    use super::*;

    fn owner() -> PersistentCallableBodyId {
        let site = SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let declaration = SourceDeclarationKey::function(
            site,
            CanonicalIdentifier::new("safepointOwner").unwrap(),
            0,
            None,
            Vec::new(),
        );
        let function = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
        PersistentCallableBodyId::from_key(&CallableBodyKey::strong(
            StrongCallableDefinitionOwner::Function(function),
        ))
        .unwrap()
    }

    #[test]
    fn identity_retains_the_complete_site_key_and_derived_runtime_id() {
        let identity =
            SafepointIdentity::new(owner(), SafepointSiteRole::ManagedInvoke, 7).unwrap();

        assert_eq!(identity.owner(), owner());
        assert_eq!(identity.role(), SafepointSiteRole::ManagedInvoke);
        assert_eq!(identity.ordinal(), 7);
        assert_eq!(
            identity.runtime_id(),
            SafepointId::derive(identity.site_id()).unwrap()
        );
    }

    #[test]
    fn relation_rejects_duplicate_local_references() {
        let reference = super::super::SafepointSiteRef::from_u32(4);
        let first = SafepointIdentity::new(owner(), SafepointSiteRole::ManagedCall, 0).unwrap();
        let second = SafepointIdentity::new(owner(), SafepointSiteRole::ManagedCall, 1).unwrap();

        assert!(matches!(
            SafepointIdentities::checked(vec![(reference, first), (reference, second)]),
            Err(SafepointIdentityRelationError::DuplicateReference { first: 0, index: 1 })
        ));
    }

    #[test]
    fn relation_rejects_duplicate_persistent_sites() {
        let identity =
            SafepointIdentity::new(owner(), SafepointSiteRole::NativeSafeTransition, 0).unwrap();

        assert!(matches!(
            SafepointIdentities::checked(vec![
                (
                    super::super::SafepointSiteRef::from_u32(0),
                    identity.clone()
                ),
                (super::super::SafepointSiteRef::from_u32(1), identity),
            ]),
            Err(SafepointIdentityRelationError::DuplicateSite { first: 0, index: 1 })
        ));
    }

    #[test]
    fn relation_rejects_runtime_id_collisions() {
        let first = SafepointIdentity::new(owner(), SafepointSiteRole::ManagedPoll, 0).unwrap();
        let mut second =
            SafepointIdentity::new(owner(), SafepointSiteRole::ManagedPoll, 1).unwrap();
        second.runtime = first.runtime;

        assert!(matches!(
            SafepointIdentities::checked(vec![
                (super::super::SafepointSiteRef::from_u32(0), first),
                (super::super::SafepointSiteRef::from_u32(1), second),
            ]),
            Err(SafepointIdentityRelationError::DuplicateRuntimeId { first: 0, index: 1 })
        ));
    }
}
