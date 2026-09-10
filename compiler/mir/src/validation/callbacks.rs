use super::*;

use std::collections::HashSet;

pub(super) fn validate_foreign_callback_metadata(
    module: &Module,
) -> Result<(), MirValidationError> {
    let mut protocol = None;
    for (family_id, family) in module.foreign_callback_families.iter() {
        let fail = |reason| MirValidationError {
            location: MirValidationLocation::ForeignCallbackFamily { family: family_id },
            kind: MirValidationErrorKind::InvalidForeignCallbackFamily { reason },
        };
        if family.callback.into_raw().into_u32() as usize >= module.structs.len() {
            return Err(fail("callback struct reference is out of bounds"));
        }
        let StructRepresentation::Declared { fields, .. } =
            &module.structs[family.callback].representation
        else {
            return Err(fail("callback type must be a declared struct"));
        };
        let [function, context] = fields.as_slice() else {
            return Err(fail(
                "callback struct must contain exact code-pointer and opaque context fields",
            ));
        };
        let Type::FunPtr(function_signature) = function.ty else {
            return Err(fail(
                "callback struct must contain exact code-pointer and opaque context fields",
            ));
        };
        if context.ty != Type::Ptr(Box::new(Type::Unit)) {
            return Err(fail(
                "callback struct must contain exact code-pointer and opaque context fields",
            ));
        }
        if function_signature.into_raw().into_u32() as usize >= module.function_types.len() {
            return Err(fail(
                "callback code-pointer signature reference is out of bounds",
            ));
        }
        if ForeignCallbackModes::checked(
            &module.enums,
            family.modes.reusable(),
            family.modes.one_shot(),
        ) != Some(family.modes)
        {
            return Err(fail("stored mode identities no longer match their enum"));
        }
        if ForeignCallbackStates::checked(
            &module.enums,
            family.states.registered(),
            family.states.active(),
            family.states.completed(),
            family.states.failed(),
        ) != Some(family.states)
        {
            return Err(fail("stored state identities no longer match their enum"));
        }
        if family.failure_result.throwable().into_raw().into_u32() as usize >= module.classes.len()
            || ForeignCallbackFailureResult::checked(
                &module.enums,
                OptionCore::checked(
                    &module.enums,
                    family.failure_result.some_payload(),
                    family.failure_result.none(),
                )
                .ok_or_else(|| fail("stored failure identities no longer match Option"))?,
                family.failure_result.throwable(),
            ) != Some(family.failure_result)
        {
            return Err(fail(
                "failure result is not the stored exact Option<Throwable> specialization",
            ));
        }

        let identity = (family.modes, family.states, family.failure_result);
        if protocol.is_some_and(|expected| expected != identity) {
            return Err(fail(
                "nominal mode/state/failure protocol conflicts with another family",
            ));
        }
        protocol = Some(identity);
    }

    let mut applications = HashSet::new();
    for (bridge_id, bridge) in module.foreign_callback_bridges.iter() {
        let fail = |reason| MirValidationError {
            location: MirValidationLocation::ForeignCallbackBridge { bridge: bridge_id },
            kind: MirValidationErrorKind::InvalidForeignCallbackBridge { reason },
        };
        if !applications.insert(bridge.application) {
            return Err(fail(
                "callback application is materialized by more than one bridge",
            ));
        }
        if bridge.family.into_raw().into_u32() as usize >= module.foreign_callback_families.len() {
            return Err(fail("family reference is out of bounds"));
        }
        let family = module.foreign_callback_families[bridge.family];
        if bridge.mode.definition(&module.enums).is_err() || !family.modes.contains(bridge.mode) {
            return Err(fail("mode does not belong to the bridge family"));
        }
        if bridge.adapter.into_raw().into_u32() as usize >= module.foreign_callback_adapters.len() {
            return Err(fail("adapter reference is out of bounds"));
        }
        let adapter = &module.foreign_callback_adapters[bridge.adapter];
        if adapter.function.into_raw().into_u32() as usize >= module.functions.len() {
            return Err(fail("adapter function reference is out of bounds"));
        }
        if adapter.managed_signature.into_raw().into_u32() as usize >= module.function_types.len() {
            return Err(fail("adapter managed signature reference is out of bounds"));
        }
        if bridge.native_signature.into_raw().into_u32() as usize >= module.function_types.len() {
            return Err(fail("native signature reference is out of bounds"));
        }
        if bridge.context_index as usize
            >= module.function_types[bridge.native_signature]
                .parameter_types
                .len()
        {
            return Err(fail("context index is outside the native signature"));
        }
    }
    Ok(())
}
