//! Canonical final undefined-symbol requirements for one verified artifact.

use std::fmt;

use super::{
    BuiltinExternalRequirementsV1, CanonicalUndefinedRelocationUseV1,
    CurrentConeUndefinedRequirementV1, RelocationTargetSlotV1, StrongDefinitionOwnerV1,
    VerifiedCurrentConeUndefinedRequirementClosureV1,
};
use crate::SlibMemberId;
use scoop_identity::{
    ConeIdentity, GeneratedBridgeUnitId, NativeExternalContractFingerprint, NativeLibraryBinding,
    OdrMemberId,
};
use scoop_lir::{
    CBridgeTargetSupportRequirementId, RuntimeSymbolContractId, TargetEhRequirementId,
    ValidatedLirTargetSelection,
};

mod wire;
pub use wire::DecodedCanonicalUndefinedSymbolRequirementSetV1;
#[cfg(test)]
use wire::DecodedFinalUndefinedSymbolRequirementV1;

mod partitions;
pub use partitions::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FinalUndefinedSymbolRequirementV1 {
    OdrMember {
        member: OdrMemberId,
    },
    IntraConeStrong {
        owner: StrongDefinitionOwnerV1,
    },

    GeneratedBridge {
        unit: GeneratedBridgeUnitId,
    },
    SourceExtern {
        contract: NativeExternalContractFingerprint,
        library: NativeLibraryBinding,
    },
    RuntimeAbi {
        contract: RuntimeSymbolContractId,
    },
    TargetEhSupport {
        contract: TargetEhRequirementId,
    },
    CBridgeTargetSupport {
        contract: CBridgeTargetSupportRequirementId,
    },
    NativeSymbol {
        symbol: Vec<u8>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalUndefinedSymbolRequirementV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    requirement: FinalUndefinedSymbolRequirementV1,
}

impl CanonicalUndefinedSymbolRequirementV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.use_site.source_member()
    }

    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }

    pub fn requirement(&self) -> FinalUndefinedSymbolRequirementV1 {
        self.requirement.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalUndefinedSymbolRequirementSetV1 {
    producer: ConeIdentity,
    selection: ValidatedLirTargetSelection,
    requirements: Vec<CanonicalUndefinedSymbolRequirementV1>,
}

impl CanonicalUndefinedSymbolRequirementSetV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub const fn selection(&self) -> ValidatedLirTargetSelection {
        self.selection
    }

    pub fn requirements(&self) -> &[CanonicalUndefinedSymbolRequirementV1] {
        &self.requirements
    }
}

pub fn finalize_undefined_symbol_requirements_v1(
    current_cone: VerifiedCurrentConeUndefinedRequirementClosureV1,
    external: BuiltinExternalRequirementsV1,
) -> CanonicalUndefinedSymbolRequirementSetV1 {
    let mut requirements = Vec::new();
    for item in current_cone.requirements() {
        let requirement = match item.requirement() {
            CurrentConeUndefinedRequirementV1::IntraConeStrong { owner } => {
                FinalUndefinedSymbolRequirementV1::IntraConeStrong { owner }
            }
            CurrentConeUndefinedRequirementV1::OdrMember { member } => {
                FinalUndefinedSymbolRequirementV1::OdrMember { member }
            }
            CurrentConeUndefinedRequirementV1::GeneratedBridge { unit } => {
                FinalUndefinedSymbolRequirementV1::GeneratedBridge { unit }
            }
        };
        requirements.push(CanonicalUndefinedSymbolRequirementV1 {
            use_site: item.use_site().clone(),
            requirement,
        });
    }

    let verified_external = &external;
    for item in verified_external.source_external_requirements() {
        requirements.push(CanonicalUndefinedSymbolRequirementV1 {
            use_site: item.use_site().clone(),
            requirement: FinalUndefinedSymbolRequirementV1::SourceExtern {
                contract: item.requirement().fingerprint(),
                library: item.requirement().library().binding(),
            },
        });
    }
    for item in verified_external.runtime_requirements() {
        requirements.push(CanonicalUndefinedSymbolRequirementV1 {
            use_site: item.use_site().clone(),
            requirement: FinalUndefinedSymbolRequirementV1::RuntimeAbi {
                contract: item.contract().id(),
            },
        });
    }
    for item in verified_external.target_eh_requirements() {
        requirements.push(CanonicalUndefinedSymbolRequirementV1 {
            use_site: item.use_site().clone(),
            requirement: FinalUndefinedSymbolRequirementV1::TargetEhSupport {
                contract: item.requirement().id(),
            },
        });
    }
    for item in verified_external.target_support_requirements() {
        requirements.push(CanonicalUndefinedSymbolRequirementV1 {
            use_site: item.use_site().clone(),
            requirement: FinalUndefinedSymbolRequirementV1::CBridgeTargetSupport {
                contract: item.requirement().id(),
            },
        });
    }
    for binding in verified_external.remaining_external_candidates() {
        requirements.push(CanonicalUndefinedSymbolRequirementV1 {
            use_site: CanonicalUndefinedRelocationUseV1::from(binding),
            requirement: FinalUndefinedSymbolRequirementV1::NativeSymbol {
                symbol: binding.symbol().to_vec(),
            },
        });
    }

    requirements.sort_unstable_by_key(|item| use_key(item.use_site()));
    CanonicalUndefinedSymbolRequirementSetV1 {
        producer: current_cone.producer(),
        selection: verified_external.selection(),
        requirements,
    }
}

type UseKey = (
    SlibMemberId,
    scoop_identity::ObjectDefinitionAtomId,
    u64,
    RelocationTargetSlotV1,
);

fn use_key(use_site: &CanonicalUndefinedRelocationUseV1) -> UseKey {
    (
        use_site.source_member(),
        use_site.containing_atom(),
        use_site.offset_within_atom(),
        use_site.target_slot(),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UndefinedSymbolRequirementFinalizationError {
    ExternalShapeImportIndexOutOfBounds { import_index: u32, imports: usize },
}

impl fmt::Display for UndefinedSymbolRequirementFinalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid final undefined requirement set: {self:?}"
        )
    }
}

impl std::error::Error for UndefinedSymbolRequirementFinalizationError {}

#[cfg(test)]
pub(in crate::link_object) mod tests;
