//! Reconciliation and closure of generated-C target-support requirements.

use std::collections::BTreeMap;
use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_lir::{CBridgeTargetSupportRequirementV1, ValidatedLirTargetSelection};

use super::{
    CanonicalUndefinedRelocationUseV1, RuntimeAbiRequirementUseV1, SourceExternalRequirementUseV1,
    StrongRelocationBindingV1, TargetEhRequirementUseV1,
    VerifiedCrossConeStrongRequirementClosureV1, VerifiedCurrentConeStrongRelocationClosureV1,
    VerifiedRuntimeAndEhRequirementClosureV1,
};
use crate::SlibMemberId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CBridgeTargetSupportRequirementUseV1 {
    use_site: CanonicalUndefinedRelocationUseV1,
    requirement: CBridgeTargetSupportRequirementV1,
}

impl CBridgeTargetSupportRequirementUseV1 {
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

pub fn classify_c_bridge_target_support_requirements_v1(
    runtime_and_eh: VerifiedRuntimeAndEhRequirementClosureV1,
    bridge_plan: &scoop_lir::GeneratedBridgePlanSetV1,
    profile: &scoop_lir::CBridgeToolchainProfileV1,
) -> Result<
    VerifiedCBridgeTargetSupportRequirementClosureV1,
    scoop_lir::CBridgeTargetSupportRegistryError,
> {
    let target = runtime_and_eh.selection().target();
    let registry = scoop_lir::CBridgeTargetSupportRegistryV1::current(target, profile)?;
    let bridges = bridge_native_contracts(&runtime_and_eh, bridge_plan);
    let mut source_external_requirements = Vec::new();
    let mut target_support_requirements = Vec::new();
    for source in runtime_and_eh
        .source_closure()
        .source_external_requirements()
    {
        let use_site = source.use_site();
        let helper = bridges
            .get(&use_site.containing_atom())
            .is_some_and(|native| *native != Some(source.requirement().fingerprint()));
        if let Some(requirement) = helper
            .then(|| registry.requirement_for_object_symbol(use_site.symbol()))
            .flatten()
        {
            target_support_requirements.push(CBridgeTargetSupportRequirementUseV1 {
                use_site: use_site.clone(),
                requirement: requirement.clone(),
            });
        } else {
            source_external_requirements.push(source.clone());
        }
    }
    let mut remaining_external_candidates = Vec::new();
    for binding in runtime_and_eh.remaining_external_candidates() {
        if let Some(requirement) = registry.requirement_for_object_symbol(binding.symbol()) {
            target_support_requirements.push(CBridgeTargetSupportRequirementUseV1 {
                use_site: CanonicalUndefinedRelocationUseV1::from(binding),
                requirement: requirement.clone(),
            });
        } else {
            remaining_external_candidates.push(binding.clone());
        }
    }
    Ok(VerifiedCBridgeTargetSupportRequirementClosureV1 {
        source_external_requirements,
        runtime_requirements: runtime_and_eh.runtime_requirements().to_vec(),
        target_eh_requirements: runtime_and_eh.target_eh_requirements().to_vec(),
        target_support_requirements,
        remaining_external_candidates,
        runtime_and_eh,
    })
}

/// Identify a bridge's native target to resolve helper/source name collisions.
fn bridge_native_contracts(
    runtime: &VerifiedRuntimeAndEhRequirementClosureV1,
    plan: &scoop_lir::GeneratedBridgePlanSetV1,
) -> BTreeMap<
    scoop_identity::ObjectDefinitionAtomId,
    Option<scoop_identity::NativeExternalContractFingerprint>,
> {
    use scoop_identity::GeneratedBridgeUnitKey;
    let recipes = plan
        .units()
        .iter()
        .map(|unit| {
            let native = match *unit.unit_authority().key() {
                GeneratedBridgeUnitKey::OutboundFunction(contract, _)
                | GeneratedBridgeUnitKey::GlobalRead(contract)
                | GeneratedBridgeUnitKey::GlobalWrite(contract)
                | GeneratedBridgeUnitKey::GlobalAddress(contract) => Some(contract),
                GeneratedBridgeUnitKey::CallbackTrampoline { .. }
                | GeneratedBridgeUnitKey::StaticCallbackTrampoline { .. } => None,
            };
            (unit.primary_atom(), native)
        })
        .collect::<BTreeMap<_, _>>();
    runtime
        .source_closure()
        .cross_cone_closure()
        .strong_closure()
        .members()
        .iter()
        .flat_map(|member| member.definitions().symbols())
        .filter_map(|symbol| {
            let super::PlannedStrongObjectSymbolRoleV1::PrimaryDefinition {
                owner,
                primary_atom,
                ..
            } = symbol.role()
            else {
                return None;
            };
            let scoop_identity::StrongDefinitionEntityKind::GeneratedBridgeAtom(atom) =
                owner.kind()
            else {
                return None;
            };
            recipes.get(&atom).map(|native| (primary_atom, *native))
        })
        .collect()
}

#[cfg(test)]
pub(in crate::link_object) fn without_target_support_for_test(
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
