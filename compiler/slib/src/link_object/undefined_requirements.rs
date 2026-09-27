//! Canonical final undefined-symbol requirements for one verified artifact.

use std::collections::BTreeSet;
use std::fmt;

use super::{
    CanonicalUndefinedRelocationUseV1, CurrentConeUndefinedRequirementV1, RelocationTargetSlotV1,
    SealedBuiltinObjectExternalRequirementClosureV1, StrongDefinitionOwnerV1,
    StrongRelocationResolutionV1, VerifiedCurrentConeUndefinedRequirementClosureV1,
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
pub(in crate::link_object) use wire::DecodedCanonicalUndefinedRelocationUseV1;
#[cfg(test)]
use wire::DecodedFinalUndefinedSymbolRequirementV1;
pub use wire::{
    DecodedCanonicalUndefinedSymbolRequirementSetV1, UndefinedSymbolRequirementValidationError,
};

mod partitions;
pub use partitions::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

    pub const fn requirement(&self) -> FinalUndefinedSymbolRequirementV1 {
        self.requirement
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

    pub(in crate::link_object) fn matches_strong_closure(
        &self,
        closure: &super::VerifiedCurrentConeStrongRelocationClosureV1,
    ) -> bool {
        if self.producer != closure.producer()
            || self.selection != ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
        {
            return false;
        }
        let expected = closure
            .bindings()
            .iter()
            .filter(|binding| {
                !matches!(
                    binding.resolution(),
                    StrongRelocationResolutionV1::ObjectLocalStrong { .. }
                )
            })
            .map(CanonicalUndefinedRelocationUseV1::from);
        let actual = self
            .requirements
            .iter()
            .map(|requirement| requirement.use_site().clone());
        actual.eq(expected)
    }
}

pub fn finalize_undefined_symbol_requirements_v1(
    current_cone: VerifiedCurrentConeUndefinedRequirementClosureV1,
    external: SealedBuiltinObjectExternalRequirementClosureV1,
) -> Result<CanonicalUndefinedSymbolRequirementSetV1, UndefinedSymbolRequirementFinalizationError> {
    let cross_cone = external.cross_cone_closure();
    if !cross_cone.semantic_imports().imports().is_empty() || !cross_cone.requirements().is_empty()
    {
        return Err(
            UndefinedSymbolRequirementFinalizationError::CrossConeRequirementsRequirePartitionedFinalizer {
                imports: cross_cone.semantic_imports().imports().len(),
                requirements: cross_cone.requirements().len(),
            },
        );
    }
    finalize_partitioned_undefined_symbol_requirements_inner(current_cone, external)
}

pub(super) fn finalize_partitioned_undefined_symbol_requirements_inner(
    current_cone: VerifiedCurrentConeUndefinedRequirementClosureV1,
    external: SealedBuiltinObjectExternalRequirementClosureV1,
) -> Result<CanonicalUndefinedSymbolRequirementSetV1, UndefinedSymbolRequirementFinalizationError> {
    finalize_partitioned_undefined_symbol_requirements_with_additional_uses_inner(
        current_cone,
        external,
        &[],
    )
}

pub(super) fn finalize_partitioned_undefined_symbol_requirements_with_additional_uses_inner(
    current_cone: VerifiedCurrentConeUndefinedRequirementClosureV1,
    external: SealedBuiltinObjectExternalRequirementClosureV1,
    additional_dependency_uses: &[CanonicalUndefinedRelocationUseV1],
) -> Result<CanonicalUndefinedSymbolRequirementSetV1, UndefinedSymbolRequirementFinalizationError> {
    let external_strong = external.verified().strong_closure();
    if current_cone.strong_closure() != external_strong {
        return Err(UndefinedSymbolRequirementFinalizationError::StrongClosureMismatch);
    }

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

    let verified_external = external.verified();
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

    requirements.sort_unstable_by_key(|item| use_key(item.use_site()));
    if let Some(pair) = requirements
        .windows(2)
        .find(|pair| use_key(pair[0].use_site()) == use_key(pair[1].use_site()))
    {
        let use_site = pair[0].use_site();
        return Err(UndefinedSymbolRequirementFinalizationError::DuplicateUse {
            member: use_site.source_member(),
            atom: use_site.containing_atom(),
            offset: use_site.offset_within_atom(),
            target_slot: use_site.target_slot(),
        });
    }

    let mut cross_cone_uses = external
        .cross_cone_closure()
        .requirements()
        .iter()
        .map(|requirement| requirement.use_site().clone())
        .collect::<Vec<_>>();
    cross_cone_uses.extend_from_slice(additional_dependency_uses);
    cross_cone_uses.sort_unstable_by_key(use_key);
    if let Some(pair) = cross_cone_uses
        .windows(2)
        .find(|pair| use_key(&pair[0]) == use_key(&pair[1]))
    {
        let use_site = &pair[0];
        return Err(
            UndefinedSymbolRequirementFinalizationError::CrossConeUseOverlap {
                member: use_site.source_member(),
                atom: use_site.containing_atom(),
                offset: use_site.offset_within_atom(),
                target_slot: use_site.target_slot(),
            },
        );
    }
    let cross_cone_keys = cross_cone_uses.iter().map(use_key).collect::<BTreeSet<_>>();
    if let Some(requirement) = requirements
        .iter()
        .find(|requirement| cross_cone_keys.contains(&use_key(requirement.use_site())))
    {
        let use_site = requirement.use_site();
        return Err(
            UndefinedSymbolRequirementFinalizationError::CrossConeUseOverlap {
                member: use_site.source_member(),
                atom: use_site.containing_atom(),
                offset: use_site.offset_within_atom(),
                target_slot: use_site.target_slot(),
            },
        );
    }

    let mut expected = current_cone
        .strong_closure()
        .bindings()
        .iter()
        .filter(|binding| {
            !matches!(
                binding.resolution(),
                StrongRelocationResolutionV1::ObjectLocalStrong { .. }
            )
        })
        .map(CanonicalUndefinedRelocationUseV1::from)
        .collect::<Vec<_>>();
    expected.sort_unstable_by_key(use_key);
    let mut actual = requirements
        .iter()
        .map(|item| item.use_site().clone())
        .chain(cross_cone_uses)
        .collect::<Vec<_>>();
    actual.sort_unstable_by_key(use_key);
    if actual != expected {
        let first_mismatch = actual
            .iter()
            .zip(&expected)
            .position(|(actual, expected)| actual != expected)
            .or_else(|| {
                (actual.len() != expected.len()).then_some(actual.len().min(expected.len()))
            });
        return Err(
            UndefinedSymbolRequirementFinalizationError::RequirementCoverageMismatch {
                expected: expected.len(),
                actual: actual.len(),
                first_mismatch,
            },
        );
    }

    Ok(CanonicalUndefinedSymbolRequirementSetV1 {
        producer: current_cone.producer(),
        selection: verified_external.selection(),
        requirements,
    })
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
    StrongClosureMismatch,
    ExternalShapeClosureMismatch,
    ExternalShapeImportIndexOutOfBounds {
        import_index: u32,
        imports: usize,
    },
    CrossConeRequirementsRequirePartitionedFinalizer {
        imports: usize,
        requirements: usize,
    },
    DuplicateUse {
        member: SlibMemberId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        offset: u64,
        target_slot: RelocationTargetSlotV1,
    },
    CrossConeUseOverlap {
        member: SlibMemberId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        offset: u64,
        target_slot: RelocationTargetSlotV1,
    },
    RequirementCoverageMismatch {
        expected: usize,
        actual: usize,
        first_mismatch: Option<usize>,
    },
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
