use std::fmt;

use scoop_identity::{
    ConeIdentity, StrongCallableDefinitionOwner, StrongDefinitionEntity, StrongDefinitionRole,
};

use crate::link_object::{
    CrossConeLinkSemanticImportBuildError, LinkDefinitionOwnerV1, StrongDefinitionOwnerV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CrossConeStrongRequirementValidationError {
    TargetMismatch {
        object: scoop_lir::LirTargetProfile,
        selection: scoop_lir::LirTargetProfile,
    },
    ConsumerMismatch {
        object: ConeIdentity,
        bridge: ConeIdentity,
    },
    SemanticImports(CrossConeLinkSemanticImportBuildError),
    SelfDependency {
        provider: ConeIdentity,
    },
    DuplicateProvider {
        provider: ConeIdentity,
    },
    MissingProvider {
        provider: ConeIdentity,
    },
    DuplicateNormalizedSymbol {
        symbol: Vec<u8>,
    },
    DuplicateUse {
        member: crate::SlibMemberId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        offset: u64,
        target_slot: crate::link_object::RelocationTargetSlotV1,
    },
    MissingDependencyOwner {
        provider: ConeIdentity,
        name: Vec<u8>,
        owner: StrongDefinitionOwnerV1,
    },
    DependencyOwnerMismatch {
        provider: ConeIdentity,
        name: Vec<u8>,
        expected: StrongDefinitionOwnerV1,
        actual: Box<LinkDefinitionOwnerV1>,
    },
    InvalidCallableOwner {
        target: StrongCallableDefinitionOwner,
    },
    InvalidExpectedOwner {
        entity: StrongDefinitionEntity,
        role: StrongDefinitionRole,
    },
}

impl fmt::Display for CrossConeStrongRequirementValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid dependency strong requirement closure: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeStrongRequirementValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SemanticImports(source) => Some(source),
            _ => None,
        }
    }
}
