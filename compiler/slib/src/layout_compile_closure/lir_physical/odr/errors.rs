use scoop_identity::{
    ConeIdentity, IdentityReferenceError, OdrGroupId, OdrMemberId, OdrMemberRole,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OdrDefinitionDifference {
    GroupKey,
    MemberKey,
    Abi,
    Lir,
    Object,
    Stackmap,
    Definition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OdrMemberConflict {
    pub first: ConeIdentity,
    pub second: ConeIdentity,
    pub group: OdrGroupId,
    pub member: OdrMemberId,
    pub role: OdrMemberRole,
    pub difference: OdrDefinitionDifference,
}

#[derive(Debug)]
pub enum OdrDefinitionMergeError {
    DuplicateArtifact(ConeIdentity),
    Identity {
        provider: ConeIdentity,
        source: IdentityReferenceError,
    },
    MissingPhysicalDefinition {
        provider: ConeIdentity,
        member: OdrMemberId,
    },
    MissingContent {
        provider: ConeIdentity,
        member: OdrMemberId,
    },
    Conflict(Box<OdrMemberConflict>),
    SymbolOwnerConflict {
        first: ConeIdentity,
        second: ConeIdentity,
        symbol: Vec<u8>,
    },
}

impl OdrDefinitionMergeError {
    pub(in crate::layout_compile_closure::lir_physical) const fn provider(&self) -> ConeIdentity {
        match self {
            Self::DuplicateArtifact(provider) => *provider,
            Self::Identity { provider, .. }
            | Self::MissingPhysicalDefinition { provider, .. }
            | Self::MissingContent { provider, .. } => *provider,
            Self::Conflict(conflict) => conflict.second,
            Self::SymbolOwnerConflict { second, .. } => *second,
        }
    }
}

impl std::fmt::Display for OdrDefinitionMergeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "incompatible cross-artifact ODR definitions: {self:?}"
        )
    }
}

impl std::error::Error for OdrDefinitionMergeError {}
