use super::*;

use std::collections::HashSet;

pub(super) fn validate_static_callback_metadata(module: &Module) -> Result<(), MirValidationError> {
    let mut sources = HashSet::new();
    let mut functions = HashSet::new();
    let mut callables = HashSet::new();
    for (bridge_id, bridge) in module.callback_bridges.iter() {
        let fail = |reason| MirValidationError {
            location: MirValidationLocation::CallbackBridge { bridge: bridge_id },
            kind: MirValidationErrorKind::InvalidCallbackBridge { reason },
        };
        let Some(source) = arena_get(&module.functions, bridge.source) else {
            return Err(fail("the source function does not exist"));
        };
        let Some(signature) = arena_get(&module.function_types, bridge.signature) else {
            return Err(fail("the callback signature does not exist"));
        };
        let Some(function) = arena_get(&module.functions, bridge.bridge_function) else {
            return Err(fail("the storage bridge function does not exist"));
        };
        if !sources.insert((bridge.source, bridge.signature)) {
            return Err(fail(
                "one source and signature have more than one callback bridge",
            ));
        }
        if !functions.insert(bridge.bridge_function) {
            return Err(fail(
                "one storage bridge function is used by more than one callback bridge",
            ));
        }
        if !callables.insert(bridge.identity().callable_record().id()) {
            return Err(fail(
                "one generated callable identity is used by more than one callback bridge",
            ));
        }

        let Some(source_materialization) = module
            .meta
            .source_callable_materializations
            .get(bridge.source)
        else {
            return Err(fail("the source function has no callable materialization"));
        };
        if bridge.identity().source() != source_materialization.materialization() {
            return Err(fail(
                "the callback bridge identifies a different source materialization",
            ));
        }
        let Some(exact_signature) = exact_static_callback_signature(module, bridge.signature)
        else {
            return Err(fail(
                "the callback signature has no complete exact type identity",
            ));
        };
        let expects_odr = source_materialization.materialization().context()
            != scoop_identity::CallableMaterializationContext::NoSubstitution;
        if bridge.identity().odr_member_record().is_some() != expects_odr {
            return Err(fail(
                "the callback bridge root does not match the source materialization context",
            ));
        }
        let odr_group = bridge
            .identity()
            .odr_member_record()
            .map(|member| member.key().group());
        let rebuilt = StaticCallbackBridgeIdentity::new(
            source_materialization.materialization(),
            exact_signature,
            odr_group,
        )
        .map_err(|_| fail("the callback bridge identity is inconsistent"))?;
        if &rebuilt != bridge.identity() {
            return Err(fail("the callback bridge identity is not canonical"));
        }

        if source.gc_effect != GcEffect::NoGc
            || signature.is_suspend
            || source.return_ty != signature.return_type
            || !source
                .params
                .iter()
                .map(|parameter| &parameter.ty)
                .eq(&signature.parameter_types)
        {
            return Err(fail(
                "the callback source does not exactly implement its no-GC signature",
            ));
        }
        let expected_parameters = static_callback_storage_parameters(signature);
        if bridge.bridge_function == bridge.source
            || function.gc_effect != GcEffect::NoGc
            || function.return_ty != Type::Unit
            || !function
                .params
                .iter()
                .map(|parameter| &parameter.ty)
                .eq(&expected_parameters)
        {
            return Err(fail(
                "the generated callback function does not implement the storage ABI",
            ));
        }
    }
    Ok(())
}

fn exact_static_callback_signature(
    module: &Module,
    signature: FunctionTypeId,
) -> Option<scoop_identity::ExactCallableSignature> {
    let signature = arena_get(&module.function_types, signature)?;
    if signature.is_suspend {
        return None;
    }
    let parameters = signature
        .parameter_types
        .iter()
        .map(|ty| {
            module
                .meta
                .source_exact_types
                .get(ty)
                .map(|exact| exact.identity_record().id())
        })
        .collect::<Option<Vec<_>>>()?;
    let result = module
        .meta
        .source_exact_types
        .get(&signature.return_type)?
        .identity_record()
        .id();
    Some(scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        parameters,
        result,
    ))
}

fn static_callback_storage_parameters(signature: &FunctionType) -> Vec<Type> {
    let mut parameters = Vec::with_capacity(
        signature.parameter_types.len() + usize::from(signature.return_type != Type::Unit),
    );
    if signature.return_type != Type::Unit {
        parameters.push(Type::Ptr(Box::new(signature.return_type.clone())));
    }
    parameters.extend(
        signature
            .parameter_types
            .iter()
            .cloned()
            .map(|parameter| Type::Ptr(Box::new(parameter))),
    );
    parameters
}

fn arena_get<T>(arena: &la_arena::Arena<T>, id: la_arena::Idx<T>) -> Option<&T> {
    ((id.into_raw().into_u32() as usize) < arena.len()).then(|| &arena[id])
}

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
        let application_id = bridge.application();
        if bridge.application_identity.id() != application_id {
            return Err(fail(
                "callback application identity and semantic record disagree",
            ));
        }
        if !applications.insert(application_id) {
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
        if !matches!(
            adapter.identity_record().key(),
            scoop_identity::GeneratedCallableKey::ForeignCallbackManagedAdapter { application }
                if *application == application_id
        ) {
            return Err(fail(
                "adapter persistent identity does not match the callback application",
            ));
        }
        let expects_odr = bridge.application_identity.key().context()
            != scoop_identity::CallableMaterializationContext::NoSubstitution;
        if adapter.odr_member_record().is_some() != expects_odr {
            return Err(fail(
                "adapter definition subject does not match the callback materialization context",
            ));
        }
        if bridge.application_record.managed_adapter() != adapter.signature_subject() {
            return Err(fail(
                "callback application record names a different managed adapter",
            ));
        }
        let managed_signature = &module.function_types[adapter.managed_signature];
        let exact_managed_signature = bridge.application_record.managed_signature();
        if managed_signature.is_suspend
            || exact_managed_signature.effect() != scoop_identity::Effect::Ordinary
            || exact_managed_signature.receiver().is_present()
            || exact_managed_signature.parameters().len() != managed_signature.parameter_types.len()
        {
            return Err(fail(
                "adapter exact managed signature has an invalid callback shape",
            ));
        }
        if bridge.native_signature.into_raw().into_u32() as usize >= module.function_types.len() {
            return Err(fail("native signature reference is out of bounds"));
        }
        let expected_callback_mode = if bridge.mode == family.modes.reusable() {
            scoop_identity::CallbackMode::Reusable
        } else {
            scoop_identity::CallbackMode::OneShot
        };
        if bridge.application_record.mode() != expected_callback_mode {
            return Err(fail(
                "persistent callback mode does not match the protocol variant",
            ));
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
