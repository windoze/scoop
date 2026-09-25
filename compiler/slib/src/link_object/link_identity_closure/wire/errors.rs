use super::*;
use std::fmt;

#[derive(Debug)]
pub enum LinkObjectMaterializationValidationError {
    Resource(WireError),
    UnknownScoopLirDefinition([u8; 32]),
    UnknownGeneratedBridgeUnit([u8; 32]),
    ScoopUnitSet(ObjectUnitSetError),
    BridgeUnitSet(ObjectUnitSetError),
    MemberPlan(LinkObjectMemberSetPlanError),
    ProjectionMismatch,
}

#[derive(Debug, Eq, PartialEq)]
pub enum LinkDigestPatchInputValidationError {
    Resource(WireError),
    DuplicateExpectedPatchIntent(DigestPatchIntentId),
    MissingTargetMember {
        intent: DigestPatchIntentId,
        definition: ObjectDefinitionPlanId,
    },
    NonScoopTargetMember {
        intent: DigestPatchIntentId,
        member: SlibMemberId,
    },
    UnknownPatchIntent([u8; 32]),
    DuplicatePatchIntent(DigestPatchIntentId),
    NonCanonicalPatchSiteOrder {
        index: usize,
    },
    MissingPatchIntent(DigestPatchIntentId),
    PatchMemberMismatch {
        intent: DigestPatchIntentId,
        expected: SlibMemberId,
    },
}

impl fmt::Display for LinkDigestPatchInputValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Link digest patch input: {self:?}")
    }
}

impl std::error::Error for LinkDigestPatchInputValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum LinkObjectProjectionValidationError {
    Resource(WireError),
    MemberPlanMismatch,
    ProjectionMismatch,
}

#[derive(Debug)]
pub enum LinkSymbolProjectionValidationError {
    DefinedSymbols(DefinedLinkSymbolOwnerValidationError),
    UndefinedSymbols(UndefinedSymbolRequirementValidationError),
}

impl fmt::Display for LinkSymbolProjectionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Link symbol projection: {self:?}")
    }
}

impl std::error::Error for LinkSymbolProjectionValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::DefinedSymbols(error) => error,
            Self::UndefinedSymbols(error) => error,
        })
    }
}

impl fmt::Display for LinkObjectProjectionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Link object projection: {self:?}")
    }
}

impl std::error::Error for LinkObjectProjectionValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::MemberPlanMismatch | Self::ProjectionMismatch => None,
        }
    }
}

impl fmt::Display for LinkObjectMaterializationValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Link object materialization: {self:?}")
    }
}

impl std::error::Error for LinkObjectMaterializationValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::ScoopUnitSet(error) | Self::BridgeUnitSet(error) => Some(error),
            Self::MemberPlan(error) => Some(error),
            Self::UnknownScoopLirDefinition(_)
            | Self::UnknownGeneratedBridgeUnit(_)
            | Self::ProjectionMismatch => None,
        }
    }
}

#[derive(Debug)]
pub enum LinkIdentityClosureSectionValidationError {
    Expected(LinkIdentityClosureBuildError),
    ProjectionMismatch,
    Encode(scoop_wire::cbor::EncodeError),
}

impl fmt::Display for LinkIdentityClosureSectionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid decoded Link identity closure: {self:?}")
    }
}

impl std::error::Error for LinkIdentityClosureSectionValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Expected(error) => Some(error),
            Self::Encode(error) => Some(error),
            Self::ProjectionMismatch => None,
        }
    }
}
