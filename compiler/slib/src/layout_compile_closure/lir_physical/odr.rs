//! Cross-artifact ODR definitions and their actual physical candidates.

use std::collections::BTreeMap;
use std::sync::Arc;

use scoop_identity::{ConeIdentity, OdrGroupId, OdrMemberId, OdrMemberKey, SpecializationKey};

use crate::{
    DefinedLinkSymbolOwnerV1, OdrAbiFingerprintV1, OdrDefinitionFingerprintV1,
    OdrMemberDirectoryEntryV1,
};

mod errors;
mod merge;

pub use errors::{OdrDefinitionDifference, OdrDefinitionMergeError, OdrMemberConflict};
pub use merge::merge_cross_cone_odr_definitions;

/// One already checked primary definition, located in its producing artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OdrDefinitionCandidate {
    provider: ConeIdentity,
    definition: DefinedLinkSymbolOwnerV1,
}

impl OdrDefinitionCandidate {
    pub const fn provider(&self) -> ConeIdentity {
        self.provider
    }

    pub const fn definition(&self) -> &DefinedLinkSymbolOwnerV1 {
        &self.definition
    }
}

/// A complete member with at least one compatible physical definition.
#[derive(Debug)]
pub struct MergedOdrMemberDefinition {
    group_key: Arc<SpecializationKey>,
    key: Arc<OdrMemberKey>,
    entry: OdrMemberDirectoryEntryV1,
    first: OdrDefinitionCandidate,
    additional: Vec<OdrDefinitionCandidate>,
}

impl MergedOdrMemberDefinition {
    pub fn group_key(&self) -> &SpecializationKey {
        &self.group_key
    }

    pub fn key(&self) -> &OdrMemberKey {
        &self.key
    }

    pub const fn member(&self) -> OdrMemberId {
        self.entry.member()
    }

    pub const fn abi(&self) -> OdrAbiFingerprintV1 {
        self.entry.abi()
    }

    pub const fn definition(&self) -> OdrDefinitionFingerprintV1 {
        self.entry.definition()
    }

    pub fn candidates(&self) -> impl Iterator<Item = &OdrDefinitionCandidate> {
        std::iter::once(&self.first).chain(&self.additional)
    }
}

/// The union of independently emitted members, with every duplicate compared.
#[derive(Debug)]
pub struct MergedOdrDefinitions {
    members: BTreeMap<(OdrGroupId, OdrMemberId), MergedOdrMemberDefinition>,
}

impl MergedOdrDefinitions {
    pub fn members(&self) -> impl ExactSizeIterator<Item = &MergedOdrMemberDefinition> {
        self.members.values()
    }

    pub fn get(
        &self,
        group: OdrGroupId,
        member: OdrMemberId,
    ) -> Option<&MergedOdrMemberDefinition> {
        self.members.get(&(group, member))
    }
}
