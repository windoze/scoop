//! Reconciliation and closure of generated-C target-support requirements.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{ConeIdentity, GeneratedBridgeUnitId};
use scoop_lir::{
    CBridgeTargetSupportRequirementV1, RuntimeSymbolContractId, TargetEhRequirementId,
    ValidatedLirTargetSelection,
};

use super::{
    CanonicalUndefinedRelocationUseV1, GeneratedBridgeRelocationSemanticV1, RelocationTargetSlotV1,
    RuntimeAbiRequirementUseV1, SourceExternalRequirementUseV1, StrongRelocationBindingV1,
    TargetEhRequirementUseV1, VerifiedCrossConeStrongRequirementClosureV1,
    VerifiedCurrentConeStrongRelocationClosureV1, VerifiedGeneratedCBridgeSemanticSetV1,
    VerifiedRuntimeAndEhRequirementClosureV1,
};
use crate::SlibMemberId;

type UseKey = (
    SlibMemberId,
    scoop_identity::ObjectDefinitionAtomId,
    u64,
    RelocationTargetSlotV1,
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CBridgeTargetSupportRequirementUseV1 {
    unit: GeneratedBridgeUnitId,
    use_site: CanonicalUndefinedRelocationUseV1,
    requirement: CBridgeTargetSupportRequirementV1,
}

impl CBridgeTargetSupportRequirementUseV1 {
    pub const fn unit(&self) -> GeneratedBridgeUnitId {
        self.unit
    }

    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }

    pub const fn requirement(&self) -> &CBridgeTargetSupportRequirementV1 {
        &self.requirement
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCBridgeTargetSupportRequirementClosureV1 {
    runtime_and_eh: VerifiedRuntimeAndEhRequirementClosureV1,
    source_external_requirements: Vec<SourceExternalRequirementUseV1>,
    runtime_requirements: Vec<RuntimeAbiRequirementUseV1>,
    target_eh_requirements: Vec<TargetEhRequirementUseV1>,
    target_support_requirements: Vec<CBridgeTargetSupportRequirementUseV1>,
    remaining_external_candidates: Vec<StrongRelocationBindingV1>,
}

impl VerifiedCBridgeTargetSupportRequirementClosureV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.runtime_and_eh.producer()
    }

    pub const fn selection(&self) -> ValidatedLirTargetSelection {
        self.runtime_and_eh.selection()
    }

    pub const fn strong_closure(&self) -> &VerifiedCurrentConeStrongRelocationClosureV1 {
        self.runtime_and_eh
            .source_closure()
            .cross_cone_closure()
            .strong_closure()
    }

    pub const fn cross_cone_closure(&self) -> &VerifiedCrossConeStrongRequirementClosureV1 {
        self.runtime_and_eh.source_closure().cross_cone_closure()
    }

    pub fn source_external_requirements(&self) -> &[SourceExternalRequirementUseV1] {
        &self.source_external_requirements
    }

    pub fn runtime_requirements(&self) -> &[RuntimeAbiRequirementUseV1] {
        &self.runtime_requirements
    }

    pub fn target_eh_requirements(&self) -> &[TargetEhRequirementUseV1] {
        &self.target_eh_requirements
    }

    pub fn target_support_requirements(&self) -> &[CBridgeTargetSupportRequirementUseV1] {
        &self.target_support_requirements
    }

    pub fn remaining_external_candidates(&self) -> &[StrongRelocationBindingV1] {
        &self.remaining_external_candidates
    }
}

pub fn verify_c_bridge_target_support_requirements_v1(
    runtime_and_eh: VerifiedRuntimeAndEhRequirementClosureV1,
    bridge_semantics: VerifiedGeneratedCBridgeSemanticSetV1,
) -> Result<
    VerifiedCBridgeTargetSupportRequirementClosureV1,
    CBridgeTargetSupportRequirementValidationError,
> {
    let strong = runtime_and_eh
        .source_closure()
        .cross_cone_closure()
        .strong_closure();
    if strong
        != bridge_semantics
            .scoop_patch_sites()
            .builtins()
            .strong_relocations()
    {
        return Err(CBridgeTargetSupportRequirementValidationError::StrongClosureMismatch);
    }
    if runtime_and_eh.selection().target() != bridge_semantics.target() {
        return Err(CBridgeTargetSupportRequirementValidationError::TargetMismatch);
    }

    let mut existing = BTreeMap::new();
    for requirement in runtime_and_eh
        .source_closure()
        .source_external_requirements()
    {
        insert_existing(
            &mut existing,
            requirement.use_site(),
            ExistingExternalRequirement::SourceExtern(requirement.requirement().fingerprint()),
        )?;
    }
    for requirement in runtime_and_eh.runtime_requirements() {
        insert_existing(
            &mut existing,
            requirement.use_site(),
            ExistingExternalRequirement::RuntimeAbi(requirement.contract().id()),
        )?;
    }
    for requirement in runtime_and_eh.target_eh_requirements() {
        insert_existing(
            &mut existing,
            requirement.use_site(),
            ExistingExternalRequirement::TargetEhSupport(requirement.requirement().id()),
        )?;
    }
    for binding in runtime_and_eh.remaining_external_candidates() {
        let use_site = CanonicalUndefinedRelocationUseV1::from(binding);
        insert_existing(
            &mut existing,
            &use_site,
            ExistingExternalRequirement::Unclassified,
        )?;
    }

    let generated_members = bridge_semantics
        .scoop_patch_sites()
        .builtins()
        .member_plan()
        .generated_bridge_members()
        .iter()
        .map(|member| member.member_id())
        .collect::<BTreeSet<_>>();
    let mut semantic_keys = BTreeSet::new();
    let mut target_support_keys = BTreeSet::new();
    let mut target_support_requirements = Vec::new();
    for semantic_use in bridge_semantics.uses() {
        let key = use_key(semantic_use.use_site());
        if !semantic_keys.insert(key) {
            return Err(
                CBridgeTargetSupportRequirementValidationError::DuplicateSemanticUse {
                    member: key.0,
                    atom: key.1,
                    offset: key.2,
                    target_slot: key.3,
                },
            );
        }
        let classified = existing.get(&key);
        match semantic_use.semantic() {
            GeneratedBridgeRelocationSemanticV1::NativeExternal { contract } => {
                require_existing(
                    semantic_use.use_site(),
                    classified,
                    ExistingExternalRequirement::SourceExtern(contract),
                )?;
            }
            GeneratedBridgeRelocationSemanticV1::RuntimeCallbackInvoke { contract } => {
                require_existing(
                    semantic_use.use_site(),
                    classified,
                    ExistingExternalRequirement::RuntimeAbi(contract),
                )?;
            }
            GeneratedBridgeRelocationSemanticV1::TargetSupport { contract } => {
                let Some(existing) = classified else {
                    return Err(semantic_mismatch(semantic_use.use_site()));
                };
                if !matches!(
                    existing.requirement,
                    ExistingExternalRequirement::SourceExtern(_)
                        | ExistingExternalRequirement::Unclassified
                ) {
                    return Err(semantic_mismatch(semantic_use.use_site()));
                }
                let requirement = bridge_semantics
                    .target_support()
                    .requirements()
                    .find(|requirement| requirement.id() == contract)
                    .ok_or(
                        CBridgeTargetSupportRequirementValidationError::MissingTargetSupportContract(
                            contract,
                        ),
                    )?;
                if existing.use_site != *semantic_use.use_site()
                    || requirement.object_symbol(bridge_semantics.target())
                        != semantic_use.use_site().symbol()
                {
                    return Err(semantic_mismatch(semantic_use.use_site()));
                }
                target_support_keys.insert(key);
                target_support_requirements.push(CBridgeTargetSupportRequirementUseV1 {
                    unit: semantic_use.unit(),
                    use_site: semantic_use.use_site().clone(),
                    requirement: requirement.clone(),
                });
            }
            GeneratedBridgeRelocationSemanticV1::StaticCallbackStorageBridge { .. }
            | GeneratedBridgeRelocationSemanticV1::SignatureDescriptor { .. } => {
                if classified.is_some() {
                    return Err(semantic_mismatch(semantic_use.use_site()));
                }
            }
        }
    }
    if let Some((key, _)) = existing
        .iter()
        .find(|(key, _)| generated_members.contains(&key.0) && !semantic_keys.contains(key))
    {
        return Err(
            CBridgeTargetSupportRequirementValidationError::MissingGeneratedBridgeSemanticUse {
                member: key.0,
                atom: key.1,
                offset: key.2,
                target_slot: key.3,
            },
        );
    }

    let source_external_requirements = runtime_and_eh
        .source_closure()
        .source_external_requirements()
        .iter()
        .filter(|requirement| !target_support_keys.contains(&use_key(requirement.use_site())))
        .cloned()
        .collect();
    let runtime_requirements = runtime_and_eh
        .runtime_requirements()
        .iter()
        .filter(|requirement| !target_support_keys.contains(&use_key(requirement.use_site())))
        .cloned()
        .collect();
    let target_eh_requirements = runtime_and_eh
        .target_eh_requirements()
        .iter()
        .filter(|requirement| !target_support_keys.contains(&use_key(requirement.use_site())))
        .cloned()
        .collect();
    let remaining_external_candidates = runtime_and_eh
        .remaining_external_candidates()
        .iter()
        .filter(|binding| {
            !target_support_keys
                .contains(&use_key(&CanonicalUndefinedRelocationUseV1::from(*binding)))
        })
        .cloned()
        .collect();

    Ok(VerifiedCBridgeTargetSupportRequirementClosureV1 {
        runtime_and_eh,
        source_external_requirements,
        runtime_requirements,
        target_eh_requirements,
        target_support_requirements,
        remaining_external_candidates,
    })
}

#[cfg(test)]
pub(in crate::link_object) fn without_generated_bridge_semantics_for_test(
    runtime_and_eh: VerifiedRuntimeAndEhRequirementClosureV1,
) -> VerifiedCBridgeTargetSupportRequirementClosureV1 {
    VerifiedCBridgeTargetSupportRequirementClosureV1 {
        source_external_requirements: runtime_and_eh
            .source_closure()
            .source_external_requirements()
            .to_vec(),
        runtime_requirements: runtime_and_eh.runtime_requirements().to_vec(),
        target_eh_requirements: runtime_and_eh.target_eh_requirements().to_vec(),
        remaining_external_candidates: runtime_and_eh.remaining_external_candidates().to_vec(),
        runtime_and_eh,
        target_support_requirements: Vec::new(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedBuiltinObjectExternalRequirementClosureV1 {
    verified: VerifiedCBridgeTargetSupportRequirementClosureV1,
}

impl SealedBuiltinObjectExternalRequirementClosureV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.verified.producer()
    }

    pub const fn verified(&self) -> &VerifiedCBridgeTargetSupportRequirementClosureV1 {
        &self.verified
    }

    pub const fn cross_cone_closure(&self) -> &VerifiedCrossConeStrongRequirementClosureV1 {
        self.verified.cross_cone_closure()
    }
}

pub fn seal_builtin_object_external_requirements_v1(
    verified: VerifiedCBridgeTargetSupportRequirementClosureV1,
) -> Result<
    SealedBuiltinObjectExternalRequirementClosureV1,
    BuiltinObjectExternalRequirementClosureError,
> {
    if let Some(binding) = verified.remaining_external_candidates().first() {
        return Err(
            BuiltinObjectExternalRequirementClosureError::UnclassifiedExternalRelocation {
                member: binding.source_member(),
                atom: binding.containing_atom(),
                symbol: binding.symbol().to_vec(),
            },
        );
    }
    Ok(SealedBuiltinObjectExternalRequirementClosureV1 { verified })
}

#[derive(Clone)]
struct ExistingExternalUse {
    use_site: CanonicalUndefinedRelocationUseV1,
    requirement: ExistingExternalRequirement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExistingExternalRequirement {
    SourceExtern(scoop_identity::NativeExternalContractFingerprint),
    RuntimeAbi(RuntimeSymbolContractId),
    TargetEhSupport(TargetEhRequirementId),
    Unclassified,
}

fn insert_existing(
    existing: &mut BTreeMap<UseKey, ExistingExternalUse>,
    use_site: &CanonicalUndefinedRelocationUseV1,
    requirement: ExistingExternalRequirement,
) -> Result<(), CBridgeTargetSupportRequirementValidationError> {
    let key = use_key(use_site);
    if existing
        .insert(
            key,
            ExistingExternalUse {
                use_site: use_site.clone(),
                requirement,
            },
        )
        .is_some()
    {
        return Err(
            CBridgeTargetSupportRequirementValidationError::OverlappingExternalClassification {
                member: key.0,
                atom: key.1,
                offset: key.2,
                target_slot: key.3,
            },
        );
    }
    Ok(())
}

fn require_existing(
    semantic_use: &CanonicalUndefinedRelocationUseV1,
    existing: Option<&ExistingExternalUse>,
    expected: ExistingExternalRequirement,
) -> Result<(), CBridgeTargetSupportRequirementValidationError> {
    if !matches!(
        existing,
        Some(actual) if actual.use_site == *semantic_use && actual.requirement == expected
    ) {
        return Err(semantic_mismatch(semantic_use));
    }
    Ok(())
}

fn semantic_mismatch(
    use_site: &CanonicalUndefinedRelocationUseV1,
) -> CBridgeTargetSupportRequirementValidationError {
    CBridgeTargetSupportRequirementValidationError::SemanticRequirementMismatch {
        member: use_site.source_member(),
        atom: use_site.containing_atom(),
        offset: use_site.offset_within_atom(),
        target_slot: use_site.target_slot(),
    }
}

fn use_key(use_site: &CanonicalUndefinedRelocationUseV1) -> UseKey {
    (
        use_site.source_member(),
        use_site.containing_atom(),
        use_site.offset_within_atom(),
        use_site.target_slot(),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CBridgeTargetSupportRequirementValidationError {
    StrongClosureMismatch,
    TargetMismatch,
    OverlappingExternalClassification {
        member: SlibMemberId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        offset: u64,
        target_slot: RelocationTargetSlotV1,
    },
    DuplicateSemanticUse {
        member: SlibMemberId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        offset: u64,
        target_slot: RelocationTargetSlotV1,
    },
    SemanticRequirementMismatch {
        member: SlibMemberId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        offset: u64,
        target_slot: RelocationTargetSlotV1,
    },
    MissingGeneratedBridgeSemanticUse {
        member: SlibMemberId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        offset: u64,
        target_slot: RelocationTargetSlotV1,
    },
    MissingTargetSupportContract(scoop_lir::CBridgeTargetSupportRequirementId),
}

impl fmt::Display for CBridgeTargetSupportRequirementValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid C-bridge target-support requirement closure: {self:?}"
        )
    }
}

impl std::error::Error for CBridgeTargetSupportRequirementValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinObjectExternalRequirementClosureError {
    UnclassifiedExternalRelocation {
        member: SlibMemberId,
        atom: scoop_identity::ObjectDefinitionAtomId,
        symbol: Vec<u8>,
    },
}

impl fmt::Display for BuiltinObjectExternalRequirementClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "incomplete built-in object external requirement closure: {self:?}"
        )
    }
}

impl std::error::Error for BuiltinObjectExternalRequirementClosureError {}

#[cfg(test)]
pub(super) mod tests;
