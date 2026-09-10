use super::*;

fn source_local_record() -> SourceLocalValueRecord {
    let site = scoop_identity::SourceDeclarationSite::new(
        scoop_identity::ConeIdentity::SINGLE_FILE,
        scoop_identity::PackagePath::root(),
        scoop_identity::DefinitionOwnerChain::top_level(),
        scoop_identity::DeclarationScope::ConeWide,
    )
    .unwrap();
    let declaration = scoop_identity::SourceDeclarationKey::function(
        site,
        scoop_identity::CanonicalIdentifier::new("sourceLocalOwner").unwrap(),
        0,
        None,
        Vec::new(),
    );
    let function =
        scoop_identity::PersistentFunctionId::from_source_declaration(&declaration).unwrap();
    scoop_identity::CborIdentityRecord::from_key(scoop_identity::LocalValueKey::new(
        scoop_identity::CallableMaterialization::new(
            scoop_identity::CallableTemplateOwner::Function(function),
            scoop_identity::CallableMaterializationContext::NoSubstitution,
        ),
        scoop_identity::LocalValueSelector::Parameter {
            declaration_index: 0,
        },
    ))
    .unwrap()
}

#[test]
fn module_validation_rejects_source_value_locations_outside_the_function_graph() {
    let (mut module, _) = module_with_variants(Vec::new());
    let missing_function = FunctionId::from_raw(7_u32.into());
    let local = LocalId::from_raw(0_u32.into());
    module.meta.source_local_values =
        SourceLocalValueIdentities::checked(vec![SourceLocalValueIdentity::new(
            missing_function,
            local,
            source_local_record(),
        )])
        .unwrap();
    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::SourceLocalValue {
                function: missing_function,
                local,
            },
            kind: MirValidationErrorKind::InvalidSourceLocalValue {
                reason: "the owning function does not exist",
            },
        })
    );

    let function = module.entry;
    module.meta.source_local_values =
        SourceLocalValueIdentities::checked(vec![SourceLocalValueIdentity::new(
            function,
            local,
            source_local_record(),
        )])
        .unwrap();
    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::SourceLocalValue { function, local },
            kind: MirValidationErrorKind::InvalidSourceLocalValue {
                reason: "the local does not exist in the owning function body",
            },
        })
    );
}

fn checked_pair(module: &Module, enum_id: EnumId) -> (MirVariantFieldRef, MirVariantRef) {
    let payload = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let payload = MirVariantFieldRef::new(&module.enums, payload, 0).unwrap();
    let empty = MirVariantRef::new(&module.enums, enum_id, 1).unwrap();
    (payload, empty)
}

fn support_function(module: &mut Module, params: Vec<Type>) -> FunctionId {
    let mut locals = Arena::new();
    let params = params
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            let name = format!("parameter{index}");
            let local = locals.alloc(Local {
                name: name.clone(),
                ty: ty.clone(),
                mutable: false,
            });
            Param { name, ty, local }
        })
        .collect();
    module.functions.alloc(Function {
        gc_effect: GcEffect::Managed,
        name: "coroutineSupport".to_string(),
        symbol: "scoop.coroutine.support".to_string(),
        params,
        return_ty: Type::Unit,
        body: Body::unreachable(locals),
    })
}

#[test]
fn module_validation_rejects_stale_option_core_metadata() {
    let value = Type::Integer(IntegerKind::SIGNED_32);
    let (mut module, enum_id) = module_with_variants(vec![
        variant_def("Some", vec![value.clone()]),
        variant_def("None", Vec::new()),
    ]);
    module.enums[enum_id].type_arguments = vec![value];
    let (some_payload, none) = checked_pair(&module, enum_id);
    module.option_core.push(
        OptionCore::checked(&module.enums, some_payload, none).expect("valid Option metadata"),
    );
    assert_eq!(module.validate(), Ok(()));

    module.enums[enum_id].variants[0].fields.clear();
    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::OptionCore {
                enumeration: enum_id,
            },
            kind: MirValidationErrorKind::InvalidOptionCore,
        })
    );
}

#[test]
fn module_validation_rejects_stale_coroutine_step_shape() {
    let result = Type::Integer(IntegerKind::SIGNED_32);
    let (mut module, enum_id) = module_with_variants(vec![
        variant_def("Completed", vec![result.clone()]),
        variant_def("Suspended", Vec::new()),
    ]);
    register_test_exact_type(&mut module, &result);
    let (completed_payload, suspended) = checked_pair(&module, enum_id);
    let identity = test_step_identity(&result);
    let step = CoroutineStep::checked(
        &module.enums,
        completed_payload,
        suspended,
        result,
        identity,
    )
    .expect("valid CoroutineStep metadata");
    let step_id = module.meta.coroutine_steps.alloc(step);
    assert_eq!(module.validate(), Ok(()));

    module.enums[enum_id].variants[0].fields.push(Field {
        name: "stale".to_string(),
        ty: Type::Boolean,
    });
    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::CoroutineStep { step: step_id },
            kind: MirValidationErrorKind::InvalidCoroutineStep,
        })
    );
}

#[test]
fn module_validation_rejects_stale_coroutine_slot_reference() {
    let value = Type::Boolean;
    let (mut module, enum_id) = module_with_variants(vec![
        variant_def("Empty", Vec::new()),
        variant_def("Value", vec![value.clone()]),
    ]);
    register_test_exact_type(&mut module, &value);
    let empty = MirVariantRef::new(&module.enums, enum_id, 0).unwrap();
    let value_variant = MirVariantRef::new(&module.enums, enum_id, 1).unwrap();
    let value_payload = MirVariantFieldRef::new(&module.enums, value_variant, 0).unwrap();
    let identity = test_slot_identity(&value);
    let slot = CoroutineSlot::checked(&module.enums, value_payload, empty, value, identity)
        .expect("valid CoroutineSlot metadata");
    let slot_id = module.meta.coroutine_slots.alloc(slot);
    assert_eq!(module.validate(), Ok(()));

    module.enums[enum_id].variants.pop();
    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::CoroutineSlot { slot: slot_id },
            kind: MirValidationErrorKind::InvalidCoroutineSlot,
        })
    );
}

#[test]
fn coroutine_support_callables_are_bound_to_the_exact_step_result() {
    let result = Type::Integer(IntegerKind::SIGNED_32);
    let (mut module, enum_id) = module_with_variants(vec![
        variant_def("Completed", vec![result.clone()]),
        variant_def("Suspended", Vec::new()),
    ]);
    register_test_exact_type(&mut module, &result);
    let (completed_payload, suspended) = checked_pair(&module, enum_id);
    module.meta.coroutine_steps.alloc(
        CoroutineStep::checked(
            &module.enums,
            completed_payload,
            suspended,
            result.clone(),
            test_step_identity(&result),
        )
        .unwrap(),
    );
    let mut interfaces = Arena::new();
    let continuation = interfaces.alloc(InterfaceDef {
        link_stem: nominal_link_stem(),
        name: "Continuation".to_string(),
        type_arguments: vec![result.clone()],
        methods: Vec::new(),
    });
    let task = interfaces.alloc(InterfaceDef {
        link_stem: nominal_link_stem(),
        name: "SuspendTask".to_string(),
        type_arguments: vec![result.clone()],
        methods: Vec::new(),
    });
    module.interfaces = interfaces;
    let throwable = module.classes.alloc(ClassDef {
        modifier: ClassModifier::Final,
        link_stem: nominal_link_stem(),
        name: "Throwable".to_string(),
        type_arguments: Vec::new(),
        representation: ClassRepresentation::Declared {
            fields: Vec::new(),
            base_class: None,
        },
        interfaces: Vec::new(),
        vtable: Vec::new(),
        itables: Vec::new(),
    });
    let success = support_function(
        &mut module,
        vec![Type::Interface(continuation), result.clone()],
    );
    let failure = support_function(
        &mut module,
        vec![Type::Interface(continuation), Type::Class(throwable)],
    );
    module.meta.continuation_shells.push(
        CoroutineContinuationShell::checked(
            &module.functions,
            result.clone(),
            success,
            failure,
            test_continuation_shell_identity(&result),
        )
        .unwrap(),
    );
    let start = support_function(
        &mut module,
        vec![Type::Interface(task), Type::Interface(continuation)],
    );
    module.top_level.push(start);
    module.meta.coroutine_starts.push(
        CoroutineStart::checked(
            &module.functions,
            result.clone(),
            start,
            test_coroutine_start_identity(&result),
        )
        .unwrap(),
    );
    assert_eq!(module.validate(), Ok(()));

    module.functions[success].return_ty = Type::Boolean;
    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::ContinuationShell { shell: 0 },
            kind: MirValidationErrorKind::InvalidCoroutineMetadata {
                reason: "continuation shells no longer have their exact generated signatures",
            },
        })
    );
}
