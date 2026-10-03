use super::*;

pub(in crate::validation) fn validate_static_callback_metadata(
    module: &Module,
) -> Result<(), MirValidationError> {
    let mut sources = HashSet::new();
    let mut functions = HashSet::new();
    let mut callables = HashSet::new();
    for (bridge_id, bridge) in module.callback_bridges.iter() {
        let fail = |reason| MirValidationError {
            location: MirValidationLocation::CallbackBridge { bridge: bridge_id },
            kind: MirValidationErrorKind::InvalidCallbackBridge { reason },
        };
        if !callables.insert(bridge.identity().callable_record().id()) {
            return Err(fail(
                "one generated callable identity is used by more than one callback bridge",
            ));
        }
        let Some((source_id, bridge_function)) = bridge.local_definition() else {
            validate_external(module, bridge).map_err(fail)?;
            continue;
        };
        let Some(source) = arena_get(&module.functions, source_id) else {
            return Err(fail("the source function does not exist"));
        };
        let Some(signature) = arena_get(&module.function_types, bridge.signature) else {
            return Err(fail("the callback signature does not exist"));
        };
        let Some(function) = arena_get(&module.functions, bridge_function) else {
            return Err(fail("the storage bridge function does not exist"));
        };
        if !sources.insert((source_id, bridge.signature)) {
            return Err(fail(
                "one source and signature have more than one callback bridge",
            ));
        }
        if !functions.insert(bridge_function) {
            return Err(fail(
                "one storage bridge function is used by more than one callback bridge",
            ));
        }

        let Some(source_materialization) =
            module.meta.source_callable_materializations.get(source_id)
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
            module
                .meta
                .source_exact_types
                .get(&Type::Unit)
                .ok_or_else(|| fail("the Unit type has no exact identity"))?
                .identity_record()
                .id(),
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
        if bridge_function == source_id
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

fn validate_external(module: &Module, bridge: &CallbackBridge) -> Result<(), &'static str> {
    let StaticCallbackTarget::External {
        source,
        bridge_function,
    } = bridge.target
    else {
        unreachable!("the caller selects external callbacks")
    };
    let source = arena_get(&module.meta.external_callables, source)
        .ok_or("the external callback source does not exist")?;
    let storage = arena_get(&module.meta.external_callables, bridge_function)
        .ok_or("the external storage bridge does not exist")?;
    if source.gc_effect() != GcEffect::NoGc
        || storage.gc_effect() != GcEffect::NoGc
        || source.reference().provider() != storage.reference().provider()
        || storage.reference().implementation()
            != scoop_identity::StrongCallableDefinitionOwner::GeneratedCallable(
                bridge.identity().callable_record().id(),
            )
    {
        return Err("the external callback does not reference its defining NoGC storage bridge");
    }
    let scoop_identity::StrongCallableDefinitionOwner::Function(function) =
        source.reference().implementation()
    else {
        return Err("the external callback source is not an ordinary function");
    };
    let source = scoop_identity::CallableMaterialization::new(
        scoop_identity::CallableTemplateOwner::Function(function),
        scoop_identity::CallableMaterializationContext::NoSubstitution,
    );
    let signature = exact_static_callback_signature(module, bridge.signature)
        .ok_or("the external callback signature is incomplete")?;
    let unit = module
        .meta
        .source_exact_types
        .get(&Type::Unit)
        .ok_or("the Unit type has no exact identity")?
        .identity_record()
        .id();
    let expected = StaticCallbackBridgeIdentity::new(source, signature, unit, None)
        .map_err(|_| "invalid external storage bridge identity")?;
    if bridge.identity() != &expected {
        return Err("the external storage bridge identity does not match its source");
    }
    Ok(())
}
