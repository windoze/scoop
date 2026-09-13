//! Producer-specific generated-C bridge definition and relocation semantics.

use std::collections::BTreeMap;

use scoop_identity::{
    GeneratedBridgeAtomId, GeneratedBridgeUnitId, NativeExternalContractFingerprint,
    PersistentCallableBodyId,
};
use scoop_lir::{
    CBridgeProductionSetV1, CBridgeTargetSupportRegistryV1, CBridgeTargetSupportRequirementId,
    CBridgeToolchainProfileV1, CanonicalNativeExternalRequirementSurfaceV1,
    GeneratedBridgePlanSetV1, LirTargetProfile, RuntimeAbiSymbolV1, RuntimeSymbolContractId,
    RuntimeSymbolContractV1,
};

use super::{CanonicalUndefinedRelocationUseV1, VerifiedBuiltinObjectStrongRelocationSetV1};

mod classification;
use classification::classify_binding;
#[cfg(test)]
use classification::{
    ObservedUnitSemantics, native_relocation_form_matches, validate_native_contract_kind,
};

mod error;
pub use error::*;

mod expected;
use expected::ExpectedBridgeSet;
#[cfg(test)]
use expected::{ExpectedBridgeUnit, derive_bridge_definition};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedBridgeRelocationSemanticV1 {
    NativeExternal {
        contract: NativeExternalContractFingerprint,
    },
    RuntimeCallbackInvoke {
        contract: RuntimeSymbolContractId,
    },
    StaticCallbackStorageBridge {
        body: PersistentCallableBodyId,
    },
    SignatureDescriptor {
        atom: GeneratedBridgeAtomId,
    },
    TargetSupport {
        contract: CBridgeTargetSupportRequirementId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedGeneratedBridgeRelocationSemanticUseV1 {
    unit: GeneratedBridgeUnitId,
    use_site: CanonicalUndefinedRelocationUseV1,
    semantic: GeneratedBridgeRelocationSemanticV1,
}

impl VerifiedGeneratedBridgeRelocationSemanticUseV1 {
    pub const fn unit(&self) -> GeneratedBridgeUnitId {
        self.unit
    }

    pub const fn use_site(&self) -> &CanonicalUndefinedRelocationUseV1 {
        &self.use_site
    }

    pub const fn semantic(&self) -> GeneratedBridgeRelocationSemanticV1 {
        self.semantic
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedGeneratedCBridgeSemanticSetV1 {
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    bridge_plan: GeneratedBridgePlanSetV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    target_support: CBridgeTargetSupportRegistryV1,
    uses: Vec<VerifiedGeneratedBridgeRelocationSemanticUseV1>,
}

impl VerifiedGeneratedCBridgeSemanticSetV1 {
    pub const fn producer(&self) -> scoop_identity::ConeIdentity {
        self.builtins.producer()
    }

    pub const fn target(&self) -> LirTargetProfile {
        self.native_requirements.target()
    }

    pub const fn builtins(&self) -> &VerifiedBuiltinObjectStrongRelocationSetV1 {
        &self.builtins
    }

    pub const fn bridge_plan(&self) -> &GeneratedBridgePlanSetV1 {
        &self.bridge_plan
    }

    pub const fn native_requirements(&self) -> &CanonicalNativeExternalRequirementSurfaceV1 {
        &self.native_requirements
    }

    pub const fn target_support(&self) -> &CBridgeTargetSupportRegistryV1 {
        &self.target_support
    }

    pub fn uses(&self) -> &[VerifiedGeneratedBridgeRelocationSemanticUseV1] {
        &self.uses
    }
}

pub fn verify_generated_c_bridge_semantics_v1(
    builtins: VerifiedBuiltinObjectStrongRelocationSetV1,
    bridge_plan: GeneratedBridgePlanSetV1,
    native_requirements: CanonicalNativeExternalRequirementSurfaceV1,
    profile: &CBridgeToolchainProfileV1,
) -> Result<VerifiedGeneratedCBridgeSemanticSetV1, GeneratedCBridgeSemanticValidationError> {
    validate_outer_contracts(&builtins, &bridge_plan, &native_requirements, profile)?;
    let target_support =
        CBridgeTargetSupportRegistryV1::current(native_requirements.target(), profile)
            .map_err(GeneratedCBridgeSemanticValidationError::TargetSupportRegistry)?;
    let runtime_callback = RuntimeSymbolContractV1::current(
        native_requirements.target(),
        RuntimeAbiSymbolV1::CallbackInvoke,
    )
    .map_err(GeneratedCBridgeSemanticValidationError::RuntimeRegistry)?;
    let expected = ExpectedBridgeSet::new(&builtins, &bridge_plan)?;
    expected.validate_actual_definitions(&builtins)?;
    expected.validate_machine_local_relocations(&builtins)?;

    let native_by_fingerprint = native_requirements
        .contracts()
        .iter()
        .map(|requirement| (requirement.fingerprint(), requirement))
        .collect::<BTreeMap<_, _>>();
    let mut state = expected.initial_state();
    let mut uses = Vec::new();
    for binding in builtins.strong_relocations().bindings() {
        let Some(source) = expected.atoms.get(&binding.containing_atom()) else {
            continue;
        };
        if binding.source_member() != source.member {
            return Err(
                GeneratedCBridgeSemanticValidationError::BridgeAtomMemberMismatch {
                    atom: binding.containing_atom(),
                    expected: source.member,
                    actual: binding.source_member(),
                },
            );
        }
        if source.role != expected::GeneratedBridgeAtomMaterializationRole::Primary {
            return Err(
                GeneratedCBridgeSemanticValidationError::AssociatedAtomRelocation {
                    unit: source.unit,
                    atom: binding.containing_atom(),
                },
            );
        }
        let unit = expected
            .units
            .get(&source.unit)
            .ok_or(GeneratedCBridgeSemanticValidationError::MissingExpectedUnit(source.unit))?;
        let semantic = classify_binding(
            binding,
            unit,
            &native_by_fingerprint,
            &runtime_callback,
            &target_support,
        )?;
        state
            .get_mut(&source.unit)
            .ok_or(GeneratedCBridgeSemanticValidationError::MissingObservedUnitState(source.unit))?
            .observe(semantic);
        uses.push(VerifiedGeneratedBridgeRelocationSemanticUseV1 {
            unit: source.unit,
            use_site: CanonicalUndefinedRelocationUseV1::from(binding),
            semantic,
        });
    }
    for (unit, observed) in state {
        let expected_unit = expected
            .units
            .get(&unit)
            .ok_or(GeneratedCBridgeSemanticValidationError::MissingExpectedUnit(unit))?;
        observed.finish(expected_unit)?;
    }
    uses.sort_unstable_by_key(|use_| {
        (
            use_.use_site.source_member(),
            use_.use_site.containing_atom(),
            use_.use_site.offset_within_atom(),
            use_.use_site.target_slot(),
        )
    });

    Ok(VerifiedGeneratedCBridgeSemanticSetV1 {
        builtins,
        bridge_plan,
        native_requirements,
        target_support,
        uses,
    })
}

fn validate_outer_contracts(
    builtins: &VerifiedBuiltinObjectStrongRelocationSetV1,
    bridge_plan: &GeneratedBridgePlanSetV1,
    native_requirements: &CanonicalNativeExternalRequirementSurfaceV1,
    profile: &CBridgeToolchainProfileV1,
) -> Result<(), GeneratedCBridgeSemanticValidationError> {
    let producer = builtins.producer();
    if bridge_plan.producer() != producer {
        return Err(
            GeneratedCBridgeSemanticValidationError::BridgeProducerMismatch {
                object: producer,
                bridge: bridge_plan.producer(),
            },
        );
    }
    if native_requirements.producer() != producer {
        return Err(
            GeneratedCBridgeSemanticValidationError::NativeProducerMismatch {
                object: producer,
                native: native_requirements.producer(),
            },
        );
    }
    match builtins.c_bridge_production().production() {
        CBridgeProductionSetV1::NotUsed => {
            if !bridge_plan.units().is_empty() {
                return Err(GeneratedCBridgeSemanticValidationError::ProductionMismatch);
            }
        }
        CBridgeProductionSetV1::Used(production) => {
            if production.profile_id() != profile.id()
                || production.profile_fingerprint() != profile.fingerprint()
                || production.units()
                    != bridge_plan
                        .units()
                        .iter()
                        .map(|unit| unit.unit())
                        .collect::<Vec<_>>()
            {
                return Err(GeneratedCBridgeSemanticValidationError::ProductionMismatch);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
