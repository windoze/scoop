use std::collections::BTreeMap;

use scoop_identity::{
    CallableBodyKey, GeneratedBridgeUnitId, GeneratedBridgeUnitKey, NativeExternAbi,
    NativeExternalContract, NativeExternalContractFingerprint, ObjectDefinitionPlanId,
    ObjectDefinitionPlanKey, PersistentCallableBodyId, StrongCallableDefinitionOwner,
    StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CBridgeTargetSupportRegistryV1, CanonicalNativeExternalRequirementV1, RuntimeSymbolContractV1,
};

use super::expected::ExpectedBridgeUnit;
use super::{GeneratedBridgeRelocationSemanticV1, GeneratedCBridgeSemanticValidationError};
use crate::{
    StrongRelocationBindingV1, StrongRelocationResolutionV1, VerifiedDarwinArm64RelocationFormV1,
};

pub(super) fn classify_binding(
    binding: &StrongRelocationBindingV1,
    unit: &ExpectedBridgeUnit,
    native_by_fingerprint: &BTreeMap<
        NativeExternalContractFingerprint,
        &CanonicalNativeExternalRequirementV1,
    >,
    runtime_callback: &RuntimeSymbolContractV1,
    target_support: &CBridgeTargetSupportRegistryV1,
) -> Result<GeneratedBridgeRelocationSemanticV1, GeneratedCBridgeSemanticValidationError> {
    match unit.key {
        GeneratedBridgeUnitKey::OutboundFunction(contract)
        | GeneratedBridgeUnitKey::GlobalRead(contract)
        | GeneratedBridgeUnitKey::GlobalWrite(contract)
        | GeneratedBridgeUnitKey::GlobalAddress(contract) => {
            let requirement = native_by_fingerprint.get(&contract).ok_or(
                GeneratedCBridgeSemanticValidationError::MissingNativeContract {
                    unit: unit.unit,
                    contract,
                },
            )?;
            validate_native_contract_kind(unit.unit, unit.key, requirement.contract())?;
            if binding.symbol() == requirement.symbol_key().native_link_symbol().as_bytes()
                && matches!(
                    binding.resolution(),
                    StrongRelocationResolutionV1::ExternalCandidate { .. }
                )
                && native_relocation_form_matches(
                    unit.key,
                    requirement.contract(),
                    binding.relocation_form(),
                )
            {
                return Ok(GeneratedBridgeRelocationSemanticV1::NativeExternal { contract });
            }
            target_support_semantic(binding, unit, target_support)?
                .ok_or_else(|| unexpected_binding(unit, binding))
        }
        GeneratedBridgeUnitKey::CallbackTrampoline { .. } => {
            if binding.symbol()
                == runtime_callback
                    .object_symbol(target_support.target())
                    .as_slice()
            {
                require_external_call(binding, unit)?;
                return Ok(GeneratedBridgeRelocationSemanticV1::RuntimeCallbackInvoke {
                    contract: runtime_callback.id(),
                });
            }
            let Some((atom, definition)) = unit.signature_descriptor else {
                return Err(
                    GeneratedCBridgeSemanticValidationError::MissingSignatureDescriptor(unit.unit),
                );
            };
            if resolved_definition(binding) == Some(definition)
                && matches!(
                    binding.relocation_form(),
                    VerifiedDarwinArm64RelocationFormV1::Unsigned64
                        | VerifiedDarwinArm64RelocationFormV1::Page21 { .. }
                        | VerifiedDarwinArm64RelocationFormV1::PageOffset12 { .. }
                        | VerifiedDarwinArm64RelocationFormV1::GotLoadPage21
                        | VerifiedDarwinArm64RelocationFormV1::GotLoadPageOffset12
                        | VerifiedDarwinArm64RelocationFormV1::PointerToGot32
                )
            {
                return Ok(GeneratedBridgeRelocationSemanticV1::SignatureDescriptor { atom });
            }
            target_support_semantic(binding, unit, target_support)?
                .ok_or_else(|| unexpected_binding(unit, binding))
        }
        GeneratedBridgeUnitKey::StaticCallbackTrampoline { storage_bridge, .. } => {
            let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(
                StrongCallableDefinitionOwner::GeneratedCallable(
                    storage_bridge.generated_callable(),
                ),
            ))
            .map_err(|_| {
                GeneratedCBridgeSemanticValidationError::InvalidStaticStorageBridgeIdentity(
                    unit.unit,
                )
            })?;
            let key = ObjectDefinitionPlanKey::strong(
                unit.producer,
                StrongDefinitionEntity::callable_body(body),
                StrongDefinitionRole::CallableBody,
            )
            .map_err(|_| {
                GeneratedCBridgeSemanticValidationError::InvalidStaticStorageBridgeIdentity(
                    unit.unit,
                )
            })?;
            let definition = ObjectDefinitionPlanId::from_key(&key).map_err(|_| {
                GeneratedCBridgeSemanticValidationError::InvalidStaticStorageBridgeIdentity(
                    unit.unit,
                )
            })?;
            if resolved_definition(binding) == Some(definition)
                && binding.relocation_form() == VerifiedDarwinArm64RelocationFormV1::Branch26
            {
                return Ok(
                    GeneratedBridgeRelocationSemanticV1::StaticCallbackStorageBridge { body },
                );
            }
            target_support_semantic(binding, unit, target_support)?
                .ok_or_else(|| unexpected_binding(unit, binding))
        }
    }
}

fn target_support_semantic(
    binding: &StrongRelocationBindingV1,
    unit: &ExpectedBridgeUnit,
    target_support: &CBridgeTargetSupportRegistryV1,
) -> Result<Option<GeneratedBridgeRelocationSemanticV1>, GeneratedCBridgeSemanticValidationError> {
    let Some(requirement) = target_support.requirement_for_object_symbol(binding.symbol()) else {
        return Ok(None);
    };
    require_external_call(binding, unit)?;
    Ok(Some(GeneratedBridgeRelocationSemanticV1::TargetSupport {
        contract: requirement.id(),
    }))
}

fn require_external_call(
    binding: &StrongRelocationBindingV1,
    unit: &ExpectedBridgeUnit,
) -> Result<(), GeneratedCBridgeSemanticValidationError> {
    if !matches!(
        binding.resolution(),
        StrongRelocationResolutionV1::ExternalCandidate { .. }
    ) || binding.relocation_form() != VerifiedDarwinArm64RelocationFormV1::Branch26
    {
        return Err(unexpected_binding(unit, binding));
    }
    Ok(())
}

pub(super) fn validate_native_contract_kind(
    unit: GeneratedBridgeUnitId,
    key: GeneratedBridgeUnitKey,
    contract: &NativeExternalContract,
) -> Result<(), GeneratedCBridgeSemanticValidationError> {
    let valid = matches!(
        (key, contract),
        (
            GeneratedBridgeUnitKey::OutboundFunction(_),
            NativeExternalContract::Function {
                abi: NativeExternAbi::C(_),
                ..
            },
        ) | (
            GeneratedBridgeUnitKey::GlobalRead(_) | GeneratedBridgeUnitKey::GlobalAddress(_),
            NativeExternalContract::ReadOnlyData { .. }
                | NativeExternalContract::MutableData { .. }
                | NativeExternalContract::ReadOnlyTls { .. }
                | NativeExternalContract::MutableTls { .. },
        ) | (
            GeneratedBridgeUnitKey::GlobalWrite(_),
            NativeExternalContract::MutableData { .. } | NativeExternalContract::MutableTls { .. },
        )
    );
    if !valid {
        return Err(GeneratedCBridgeSemanticValidationError::NativeContractKindMismatch(unit));
    }
    Ok(())
}

pub(super) fn native_relocation_form_matches(
    key: GeneratedBridgeUnitKey,
    contract: &NativeExternalContract,
    form: VerifiedDarwinArm64RelocationFormV1,
) -> bool {
    match key {
        GeneratedBridgeUnitKey::OutboundFunction(_) => {
            form == VerifiedDarwinArm64RelocationFormV1::Branch26
        }
        GeneratedBridgeUnitKey::GlobalRead(_)
        | GeneratedBridgeUnitKey::GlobalWrite(_)
        | GeneratedBridgeUnitKey::GlobalAddress(_) => match contract {
            NativeExternalContract::ReadOnlyTls { .. }
            | NativeExternalContract::MutableTls { .. } => matches!(
                form,
                VerifiedDarwinArm64RelocationFormV1::TlvpLoadPage21
                    | VerifiedDarwinArm64RelocationFormV1::TlvpLoadPageOffset12
            ),
            NativeExternalContract::ReadOnlyData { .. }
            | NativeExternalContract::MutableData { .. } => matches!(
                form,
                VerifiedDarwinArm64RelocationFormV1::Unsigned64
                    | VerifiedDarwinArm64RelocationFormV1::Page21 { .. }
                    | VerifiedDarwinArm64RelocationFormV1::PageOffset12 { .. }
                    | VerifiedDarwinArm64RelocationFormV1::GotLoadPage21
                    | VerifiedDarwinArm64RelocationFormV1::GotLoadPageOffset12
                    | VerifiedDarwinArm64RelocationFormV1::PointerToGot32
            ),
            NativeExternalContract::Function { .. } => false,
        },
        GeneratedBridgeUnitKey::CallbackTrampoline { .. }
        | GeneratedBridgeUnitKey::StaticCallbackTrampoline { .. } => false,
    }
}

fn resolved_definition(binding: &StrongRelocationBindingV1) -> Option<ObjectDefinitionPlanId> {
    match binding.resolution() {
        StrongRelocationResolutionV1::ObjectLocalStrong { definition, .. }
        | StrongRelocationResolutionV1::CurrentConeUndefinedStrong { definition, .. } => {
            Some(definition)
        }
        StrongRelocationResolutionV1::ExternalCandidate { .. } => None,
    }
}

fn unexpected_binding(
    unit: &ExpectedBridgeUnit,
    binding: &StrongRelocationBindingV1,
) -> GeneratedCBridgeSemanticValidationError {
    GeneratedCBridgeSemanticValidationError::UnexpectedPrimaryRelocation {
        unit: unit.unit,
        atom: binding.containing_atom(),
        symbol: binding.symbol().to_vec(),
    }
}

#[derive(Default)]
pub(super) struct ObservedUnitSemantics {
    native: bool,
    runtime: bool,
    storage: bool,
    signature_descriptor: bool,
}

impl ObservedUnitSemantics {
    pub(super) fn observe(&mut self, semantic: GeneratedBridgeRelocationSemanticV1) {
        match semantic {
            GeneratedBridgeRelocationSemanticV1::NativeExternal { .. } => self.native = true,
            GeneratedBridgeRelocationSemanticV1::RuntimeCallbackInvoke { .. } => {
                self.runtime = true
            }
            GeneratedBridgeRelocationSemanticV1::StaticCallbackStorageBridge { .. } => {
                self.storage = true
            }
            GeneratedBridgeRelocationSemanticV1::SignatureDescriptor { .. } => {
                self.signature_descriptor = true
            }
            GeneratedBridgeRelocationSemanticV1::TargetSupport { .. } => {}
        }
    }

    pub(super) fn finish(
        self,
        unit: &ExpectedBridgeUnit,
    ) -> Result<(), GeneratedCBridgeSemanticValidationError> {
        let complete = match unit.key {
            GeneratedBridgeUnitKey::OutboundFunction(_)
            | GeneratedBridgeUnitKey::GlobalRead(_)
            | GeneratedBridgeUnitKey::GlobalWrite(_)
            | GeneratedBridgeUnitKey::GlobalAddress(_) => self.native,
            GeneratedBridgeUnitKey::CallbackTrampoline { .. } => {
                self.runtime && self.signature_descriptor
            }
            GeneratedBridgeUnitKey::StaticCallbackTrampoline { .. } => self.storage,
        };
        if !complete {
            return Err(
                GeneratedCBridgeSemanticValidationError::MissingRequiredRelocation(unit.unit),
            );
        }
        Ok(())
    }
}
