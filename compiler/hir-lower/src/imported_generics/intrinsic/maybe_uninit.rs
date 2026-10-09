use super::*;
use crate::call_resolution::candidates::ValueParameterCalling;

impl ImportedIntrinsicSignature {
    pub(super) fn validate_maybe_uninit(&self, state: &Lowerer) -> Result<(), String> {
        let hir::CallableImplementationV1::Intrinsic(hir::IntrinsicFunctionKind::MaybeUninit(kind)) =
            self.declaration.interface().effects().implementation()
        else {
            return Ok(());
        };
        if self.maybe_uninit_signature_matches(state, kind) {
            Ok(())
        } else {
            Err(format!(
                "malformed imported MaybeUninit intrinsic `{}`",
                kind.name()
            ))
        }
    }

    fn maybe_uninit_signature_matches(
        &self,
        state: &Lowerer,
        kind: hir::MaybeUninitIntrinsic,
    ) -> bool {
        let signature = &self.signature.signature;
        let Some(receiver) = self.signature.receiver else {
            return false;
        };
        let wrapper = match kind {
            hir::MaybeUninitIntrinsic::AssumeInit => receiver,
            _ => signature.return_type,
        };
        let Some(payload) = state.maybe_uninit_value_type(wrapper) else {
            return false;
        };
        let Some(application) = state.nominal_application(wrapper) else {
            return false;
        };
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = self.declaration.interface().owner()
        else {
            return false;
        };
        let owner_matches = match kind {
            hir::MaybeUninitIntrinsic::AssumeInit => owner == application.template,
            _ => {
                state.nominal_is_companion(owner)
                    && state
                        .dependencies
                        .as_ref()
                        .and_then(|dependencies| {
                            dependencies.nominal_declaration(application.template)
                        })
                        .is_some_and(|host| {
                            host.interface
                                .declaration_details()
                                .children()
                                .values()
                                .contains(&owner)
                        })
            }
        };
        let effects = self.declaration.interface().effects();
        let safety = if kind == hir::MaybeUninitIntrinsic::AssumeInit {
            hir::CallableSafetyV1::Unsafe
        } else {
            hir::CallableSafetyV1::Safe
        };
        if !owner_matches
            || signature.owner_parameters.len() != 1
            || !signature.callable_parameters.is_empty()
            || state.types[payload] != hir::Type::Param(signature.owner_parameters[0].id)
            || effects.execution() != scoop_identity::Effect::Ordinary
            || effects.gc_effect() != scoop_identity::GcEffect::NoGc
            || effects.safety() != safety
            || self.declaration.name().rsplit('.').next() != Some(kind.source_name())
        {
            return false;
        }
        match kind {
            hir::MaybeUninitIntrinsic::Uninit => signature.value_parameters.is_empty(),
            hir::MaybeUninitIntrinsic::AssumeInit => {
                signature.value_parameters.is_empty() && signature.return_type == payload
            }
            hir::MaybeUninitIntrinsic::Initialized => {
                matches!(signature.value_parameters.as_slice(), [parameter] if parameter.name == "value" && parameter.ty == payload && matches!(parameter.calling, ValueParameterCalling::Required))
            }
        }
    }
}
