use super::*;
use scoop_identity::{
    CallableMaterializationContext, CallableTemplateOwner, InitializationCallableRole,
    InitializationUnitKey,
};

impl MirCallableBridgeAuthority<'_> {
    pub(super) fn validate_generated(
        &self,
        binding: &ParamFreeMirCallableBindingV1,
        generated: &GeneratedCallableKey,
    ) -> Result<(), MirCallableBridgeError> {
        let semantic = binding.semantic.exact();
        let lowered = binding.lowered.exact();
        match (generated, binding.role) {
            (
                GeneratedCallableKey::Initialization { unit: actual, role },
                MirCallableLoweringRoleV1::ObjectEnsure { unit },
            ) if *actual == unit && *role == InitializationCallableRole::Ensure => {
                self.object_initializer(binding, unit)
            }
            (
                GeneratedCallableKey::Initialization { unit: actual, role },
                MirCallableLoweringRoleV1::ObjectInitializer { unit },
            ) if *actual == unit && *role == InitializationCallableRole::Initializer => {
                self.object_initializer(binding, unit)
            }
            (
                GeneratedCallableKey::DispatchAdjust {
                    slot,
                    implementor,
                    target: key_target,
                },
                MirCallableLoweringRoleV1::DispatchAdjust { target },
            ) => {
                let expected = match target {
                    StrongCallableDefinitionOwner::Function(id) => {
                        CallableTemplateOwner::Function(id)
                    }
                    StrongCallableDefinitionOwner::PropertyAccessor(id) => {
                        CallableTemplateOwner::Accessor(id)
                    }
                    _ => return Err(MirCallableBridgeError::InvalidAdjustTarget),
                };
                if key_target.context() != CallableMaterializationContext::NoSubstitution
                    || key_target.template() != expected
                {
                    return Err(MirCallableBridgeError::InvalidAdjustTarget);
                }
                self.adjust(binding, *slot, *implementor, target)
            }
            (
                GeneratedCallableKey::BoxingAdjust {
                    slot,
                    payload,
                    interface,
                },
                MirCallableLoweringRoleV1::BoxingAdjust { target },
            ) => {
                if self.type_export(*payload)?.facts().kind() == MirValueKindV1::Reference
                    || !matches!(
                        self.type_export(*interface)?.representation(),
                        MirTypeRepresentationV1::Interface
                    )
                    || lowered.receiver() != OptionalExactOwner::Present(*interface)
                {
                    return Err(MirCallableBridgeError::InvalidAdjustTarget);
                }
                let receiver = semantic
                    .receiver()
                    .into_option()
                    .ok_or(MirCallableBridgeError::InvalidAdjustTarget)?;
                if receiver != *payload
                    && !matches!(
                        self.type_export(receiver)?.representation(),
                        MirTypeRepresentationV1::Interface
                    )
                {
                    return Err(MirCallableBridgeError::InvalidAdjustTarget);
                }
                // A default body receives the interface view of the same box.
                // The dispatch schema proves the payload-to-interface path.
                self.adjust(binding, *slot, receiver, target)
            }
            (
                GeneratedCallableKey::DerivedEquality { exact_owner },
                MirCallableLoweringRoleV1::DerivedEquality { owner },
            ) if *exact_owner == owner => {
                self.same_signatures(binding)?;
                if self.type_export(owner)?.facts().kind() == MirValueKindV1::Reference {
                    return Err(MirCallableBridgeError::SignatureMismatch);
                }
                if semantic.receiver() != OptionalExactOwner::Present(owner)
                    || semantic.parameters() != [owner]
                    || !matches!(
                        self.type_export(semantic.result())?.representation(),
                        MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Boolean)
                    )
                {
                    return Err(MirCallableBridgeError::SignatureMismatch);
                }
                Ok(())
            }
            (
                GeneratedCallableKey::Initialization { .. }
                | GeneratedCallableKey::DerivedEquality { .. }
                | GeneratedCallableKey::DispatchAdjust { .. }
                | GeneratedCallableKey::BoxingAdjust { .. },
                _,
            ) => Err(MirCallableBridgeError::RoleMismatch),
            _ => {
                let StrongCallableDefinitionOwner::GeneratedCallable(callable) =
                    binding.implementation
                else {
                    return Err(MirCallableBridgeError::OriginMismatch);
                };
                Err(MirCallableBridgeError::GeneratedExecutionGate { callable })
            }
        }
    }
    fn object_initializer(
        &self,
        binding: &ParamFreeMirCallableBindingV1,
        unit: PersistentInitializationUnitId,
    ) -> Result<(), MirCallableBridgeError> {
        self.same_signatures(binding)?;
        let key = self
            .identities
            .canonical_key::<_, InitializationUnitKey>(unit)?;
        let object = match key.as_ref() {
            InitializationUnitKey::Object(object) | InitializationUnitKey::Companion(object) => {
                *object
            }
            _ => return Err(MirCallableBridgeError::InvalidInitializationUnit),
        };
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(object))
            .map_err(|_| MirCallableBridgeError::InvalidInitializationUnit)?;
        if !matches!(
            self.type_export(exact)?.representation(),
            MirTypeRepresentationV1::Object { .. }
        ) {
            return Err(MirCallableBridgeError::InvalidInitializationUnit);
        }
        let signature = binding.lowered.exact();
        if binding.lowered.gc_effect() != crate::GcEffect::Managed
            || signature.receiver().is_present()
            || !signature.parameters().is_empty()
            || !self.is_unit(signature.result())?
        {
            return Err(MirCallableBridgeError::SignatureMismatch);
        }
        Ok(())
    }
    fn adjust(
        &self,
        binding: &ParamFreeMirCallableBindingV1,
        slot: PersistentDispatchSlotId,
        implementor: PersistentExactTypeId,
        target: StrongCallableDefinitionOwner,
    ) -> Result<(), MirCallableBridgeError> {
        if !matches!(
            target,
            StrongCallableDefinitionOwner::Function(_)
                | StrongCallableDefinitionOwner::PropertyAccessor(_)
        ) {
            return Err(MirCallableBridgeError::InvalidAdjustTarget);
        }
        let semantic = binding.semantic.exact();
        let lowered = binding.lowered.exact();
        self.identities.canonical_key::<_, DispatchSlotKey>(slot)?;
        if self.foundation_signature(target)? != semantic
            || semantic.receiver() != OptionalExactOwner::Present(implementor)
            || !lowered.receiver().is_present()
            || semantic.parameters() != lowered.parameters()
            || semantic.result() != lowered.result()
        {
            return Err(MirCallableBridgeError::InvalidAdjustTarget);
        }
        Ok(())
    }
}
