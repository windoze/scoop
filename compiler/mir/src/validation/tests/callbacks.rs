use super::*;

use scoop_identity::{
    CallableApplicationKey, CallableInstantiationOwner, CallableMaterialization,
    CallableMaterializationContext, CallableOwner, CallableTemplateOwner, CallbackApplicationKey,
    CallbackMode, CallbackParameterIndex, CallbackRegistrationKey, CanonicalIdentifier,
    CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DeclarationScope, DefinitionOwnerChain,
    Effect, ExactCallableSignature, ExactTypeKey, GeneratedCallableKey, LexicalCallableParent,
    OdrGroupId, OdrMemberDiscriminator, OdrMemberKey, OdrMemberRole, PackagePath,
    PersistentCallableApplicationId, PersistentCallbackApplicationId, PersistentExactTypeId,
    PersistentFunctionId, PersistentGeneratedCallableId, SignatureCallableShape, SignatureTypeKey,
    SourceCAbiFunctionSignature, SourceCAbiReturn, SourceDeclarationKey, SourceDeclarationSite,
    SpecializationKey, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

fn unit_variant(name: &str) -> VariantDef {
    VariantDef {
        name: name.to_string(),
        gc_free: true,
        fields: Vec::new(),
    }
}

fn named_callback_owner(name: &str) -> PersistentFunctionId {
    let site = SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap()
}

fn callback_owner() -> PersistentFunctionId {
    named_callback_owner("callbackOwner")
}

fn source_materialization(name: &str) -> CallableMaterialization {
    CallableMaterialization::new(
        CallableTemplateOwner::Function(named_callback_owner(name)),
        CallableMaterializationContext::NoSubstitution,
    )
}

fn callback_application_with_context(
    ordinal: u32,
    context: CallableMaterializationContext,
) -> CborIdentityRecord<PersistentCallbackApplicationId, CallbackApplicationKey> {
    let function = callback_owner();
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let registration = CallbackRegistrationKey::new(
        LexicalCallableParent::function(function),
        StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::CallbackConversion, ordinal),
            [],
        ),
        SourceCAbiFunctionSignature::new(Vec::new(), SourceCAbiReturn::Void),
        CallbackParameterIndex::new(0),
        SignatureCallableShape::new(
            Effect::Ordinary,
            None,
            Vec::new(),
            SignatureTypeKey::Nominal(unit),
        ),
        CallbackMode::Reusable,
    );
    let application = CallbackApplicationKey::new(&registration, context).unwrap();
    CborIdentityRecord::from_key(application).unwrap()
}

fn callback_application(
    ordinal: u32,
) -> CborIdentityRecord<PersistentCallbackApplicationId, CallbackApplicationKey> {
    callback_application_with_context(ordinal, CallableMaterializationContext::NoSubstitution)
}

fn exact_callback_signature() -> ExactCallableSignature {
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(unit)).unwrap();
    ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), unit)
}

fn static_callback_module() -> (Module, CallbackBridgeId) {
    let (mut module, _) = module_with_variants(Vec::new());
    register_test_exact_type(&mut module, &Type::Unit);
    let signature = module.function_types.alloc(FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: Type::Unit,
    });
    let bridge_function = module.functions.alloc(Function {
        gc_effect: GcEffect::NoGc,
        name: "static callback bridge".to_string(),
        symbol: "scoop.static.callback.bridge".to_string(),
        params: Vec::new(),
        return_ty: Type::Unit,
        body: Body::unreachable(Arena::new()),
    });
    module.top_level.push(bridge_function);
    let materialization = source_materialization("staticCallbackSource");
    module.meta.source_callable_materializations = SourceCallableMaterializations::checked(vec![
        SourceCallableMaterialization::new(
            module.entry,
            materialization,
            exact_callback_signature(),
            None,
        )
        .unwrap(),
    ])
    .unwrap();
    let bridge = module.callback_bridges.alloc(
        CallbackBridge::new(
            module.entry,
            signature,
            bridge_function,
            materialization,
            exact_callback_signature(),
            None,
        )
        .unwrap(),
    );
    install_generated_callables(&mut module);
    (module, bridge)
}

#[test]
fn static_callback_bridge_has_one_canonical_source_identity() {
    let (module, _) = static_callback_module();

    module.validate().unwrap();
}

#[test]
fn static_callback_bridge_must_name_its_source_materialization() {
    let (mut module, bridge) = static_callback_module();
    let source = module.callback_bridges[bridge].source;
    let signature = module.callback_bridges[bridge].signature;
    let bridge_function = module.callback_bridges[bridge].bridge_function;
    module.callback_bridges[bridge] = CallbackBridge::new(
        source,
        signature,
        bridge_function,
        source_materialization("differentStaticCallbackSource"),
        exact_callback_signature(),
        None,
    )
    .unwrap();

    assert_eq!(
        module.validate().unwrap_err(),
        MirValidationError {
            location: MirValidationLocation::CallbackBridge { bridge },
            kind: MirValidationErrorKind::InvalidCallbackBridge {
                reason: "the callback bridge identifies a different source materialization",
            },
        }
    );
}

#[test]
fn static_callback_bridge_requires_the_exact_storage_abi() {
    let (mut module, bridge) = static_callback_module();
    let function = module.callback_bridges[bridge].bridge_function;
    module.functions[function].gc_effect = GcEffect::Managed;

    assert_eq!(
        module.validate().unwrap_err(),
        MirValidationError {
            location: MirValidationLocation::CallbackBridge { bridge },
            kind: MirValidationErrorKind::InvalidCallbackBridge {
                reason: "the generated callback function does not implement the storage ABI",
            },
        }
    );
}

fn callback_module() -> (Module, ForeignCallbackFamilyId, ForeignCallbackBridgeId) {
    let (mut module, _) = module_with_variants(vec![unit_variant("Unused")]);
    register_test_exact_type(&mut module, &Type::Unit);
    let signature = module.function_types.alloc(FunctionType {
        is_suspend: false,
        parameter_types: Vec::new(),
        return_type: Type::Unit,
    });
    let callback = module.structs.alloc(StructDef {
        link_stem: nominal_link_stem(),
        type_arguments: Vec::new(),
        name: "ForeignCallback<() -> Unit>".to_string(),
        gc_free: true,
        representation: StructRepresentation::Declared {
            c_layout: None,
            interior_mutable: false,
            fields: vec![
                Field {
                    name: "function".to_string(),
                    ty: Type::FunPtr(signature),
                },
                Field {
                    name: "context".to_string(),
                    ty: Type::Ptr(Box::new(Type::Unit)),
                },
            ],
        },
    });
    register_test_exact_type(&mut module, &Type::Struct(callback));
    let throwable = module.classes.alloc(ClassDef {
        link_stem: nominal_link_stem(),
        type_arguments: Vec::new(),
        modifier: ClassModifier::Open,
        name: "Throwable".to_string(),
        representation: ClassRepresentation::Declared {
            fields: Vec::new(),
            base_class: None,
        },
        interfaces: Vec::new(),
        vtable: Vec::new(),
        itables: Vec::new(),
    });
    register_test_exact_type(&mut module, &Type::Class(throwable));
    let mode = module.enums.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "ForeignCallbackMode".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: [unit_variant("Reusable"), unit_variant("OneShot")].into(),
    });
    let state = module.enums.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "ForeignCallbackState".to_string(),
        type_arguments: Vec::new(),
        gc_free: true,
        variants: [
            unit_variant("Registered"),
            unit_variant("Active"),
            unit_variant("Completed"),
            unit_variant("Failed"),
        ]
        .into(),
    });
    let failure = module.enums.alloc(EnumDef {
        link_stem: nominal_link_stem(),
        name: "Option<Throwable>".to_string(),
        type_arguments: vec![Type::Class(throwable)],
        gc_free: false,
        variants: vec![
            variant_def("Some", vec![Type::Class(throwable)]),
            unit_variant("None"),
        ],
    });
    register_test_exact_type(&mut module, &Type::Enum(mode, Vec::new()));
    register_test_exact_type(&mut module, &Type::Enum(state, Vec::new()));
    register_test_exact_type(
        &mut module,
        &Type::Enum(failure, vec![Type::Class(throwable)]),
    );
    let modes = ForeignCallbackModes::checked(
        &module.enums,
        MirVariantRef::new(&module.enums, mode, 0).unwrap(),
        MirVariantRef::new(&module.enums, mode, 1).unwrap(),
    )
    .unwrap();
    let states = ForeignCallbackStates::checked(
        &module.enums,
        MirVariantRef::new(&module.enums, state, 0).unwrap(),
        MirVariantRef::new(&module.enums, state, 1).unwrap(),
        MirVariantRef::new(&module.enums, state, 2).unwrap(),
        MirVariantRef::new(&module.enums, state, 3).unwrap(),
    )
    .unwrap();
    let some = MirVariantRef::new(&module.enums, failure, 0).unwrap();
    let option = OptionCore::checked(
        &module.enums,
        MirVariantFieldRef::new(&module.enums, some, 0).unwrap(),
        MirVariantRef::new(&module.enums, failure, 1).unwrap(),
    )
    .unwrap();
    let failure_result =
        ForeignCallbackFailureResult::checked(&module.enums, option, throwable).unwrap();
    let family = module
        .foreign_callback_families
        .alloc(ForeignCallbackFamily {
            callback,
            modes,
            states,
            failure_result,
        });
    let application_identity = callback_application(0);
    let application = application_identity.id();
    let exact_signature = exact_callback_signature();
    let adapter_function = module.functions.alloc(Function {
        gc_effect: GcEffect::Managed,
        name: "foreign callback adapter".to_string(),
        symbol: "scoop.foreign.callback.adapter".to_string(),
        params: Vec::new(),
        return_ty: Type::Unit,
        body: Body::unreachable(Arena::new()),
    });
    let adapter = module.foreign_callback_adapters.alloc(
        ForeignCallbackAdapter::checked(
            adapter_function,
            signature,
            &module.function_types[signature],
            application,
            &exact_signature,
            None,
        )
        .unwrap(),
    );
    let native_signature = module.function_types.alloc(FunctionType {
        is_suspend: false,
        parameter_types: vec![Type::Ptr(Box::new(Type::Unit))],
        return_type: Type::Unit,
    });
    let bridge = module
        .foreign_callback_bridges
        .alloc(ForeignCallbackBridge {
            application_identity,
            application_record: CallbackApplicationRecord::new(
                application,
                module.foreign_callback_adapters[adapter].signature_subject(),
                exact_signature,
                ForeignCallbackStorageAbi::ClosureResultRootsThrowableToU32,
                CallbackMode::Reusable,
            ),
            adapter,
            family,
            native_signature,
            context_index: 0,
            mode: modes.reusable(),
        });
    install_generated_callables(&mut module);
    (module, family, bridge)
}

#[test]
fn every_mir_generated_callable_requires_a_function_materialization() {
    let (mut module, _) = static_callback_module();
    module.meta.generated_callables = MirGeneratedCallableIdentities::default();

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::GeneratedCallable { entry: 0 },
            kind: MirValidationErrorKind::InvalidGeneratedCallable {
                reason: "a MIR-generated callable has no typed function materialization",
            },
        })
    );
}

#[test]
fn generated_callable_function_must_retain_its_signature_subject() {
    let (mut module, bridge) = static_callback_module();
    let generated = module
        .meta
        .generated_callables
        .get(module.callback_bridges[bridge].bridge_function)
        .unwrap();
    let function = generated.function();
    let identity = generated.identity_record().clone();
    let generated_subject = generated.signature_subject();
    let source_subject = module
        .meta
        .source_callable_materializations
        .get(module.entry)
        .unwrap()
        .signature_record()
        .subject();
    assert_eq!(
        module.meta.callable_signature_subject(function),
        Some(generated_subject)
    );
    assert_eq!(
        module.meta.callable_signature_subject(module.entry),
        Some(source_subject)
    );
    module.meta.generated_callables =
        MirGeneratedCallableIdentities::checked(vec![MirGeneratedCallableIdentity::new(
            function,
            &identity,
            source_subject,
        )])
        .unwrap();

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::GeneratedCallable { entry: 1 },
            kind: MirValidationErrorKind::InvalidGeneratedCallable {
                reason: "the function materialization names a different signature subject",
            },
        })
    );
}

#[test]
fn one_function_cannot_carry_source_and_generated_callable_identities() {
    let (mut module, bridge) = static_callback_module();
    module.meta.generated_callables =
        MirGeneratedCallableIdentities::checked(vec![MirGeneratedCallableIdentity::new(
            module.entry,
            module.callback_bridges[bridge].identity().callable_record(),
            module.callback_bridges[bridge]
                .identity()
                .signature_record()
                .subject(),
        )])
        .unwrap();

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::GeneratedCallable { entry: 0 },
            kind: MirValidationErrorKind::InvalidGeneratedCallable {
                reason: "the function is also claimed by a source callable materialization",
            },
        })
    );
}

#[test]
fn callback_application_has_one_mir_bridge() {
    let (mut module, _, bridge) = callback_module();
    let source = &module.foreign_callback_bridges[bridge];
    let duplicate = ForeignCallbackBridge {
        application_identity: source.application_identity.clone(),
        application_record: source.application_record.clone(),
        adapter: source.adapter,
        family: source.family,
        native_signature: source.native_signature,
        context_index: source.context_index,
        mode: source.mode,
    };
    let duplicate = module.foreign_callback_bridges.alloc(duplicate);

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::ForeignCallbackBridge { bridge: duplicate },
            kind: MirValidationErrorKind::InvalidForeignCallbackBridge {
                reason: "callback application is materialized by more than one bridge",
            },
        })
    );
}

#[test]
fn callback_application_identity_must_match_its_semantic_record() {
    let (mut module, _, bridge) = callback_module();
    module.foreign_callback_bridges[bridge].application_identity = callback_application(1);

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::ForeignCallbackBridge { bridge },
            kind: MirValidationErrorKind::InvalidForeignCallbackBridge {
                reason: "callback application identity and semantic record disagree",
            },
        })
    );
}

#[test]
fn callback_application_record_must_name_its_adapter_subject() {
    let (mut module, _, bridge) = callback_module();
    let record = module.foreign_callback_bridges[bridge]
        .application_record
        .clone();
    module.foreign_callback_bridges[bridge].application_record = CallbackApplicationRecord::new(
        record.application(),
        CallableSignatureSubject::strong(CallableOwner::Function(callback_owner())),
        record.managed_signature().clone(),
        record.storage_abi(),
        record.mode(),
    );

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::ForeignCallbackBridge { bridge },
            kind: MirValidationErrorKind::InvalidForeignCallbackBridge {
                reason: "callback application record names a different managed adapter",
            },
        })
    );
}

#[test]
fn callback_family_metadata_revalidates_every_typed_role() {
    let (mut module, family, _) = callback_module();
    assert!(module.validate().is_ok(), "{:?}", module.validate());

    let state = module.foreign_callback_families[family].states.enum_id();
    module.enums[state].variants.swap(0, 1);
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::ForeignCallbackFamily { family: found },
            kind: MirValidationErrorKind::InvalidForeignCallbackFamily { .. },
        }) if found == family
    ));
}

#[test]
fn callback_bridge_mode_must_belong_to_its_family() {
    let (mut module, family, bridge) = callback_module();
    module.foreign_callback_bridges[bridge].mode =
        module.foreign_callback_families[family].states.registered();
    assert!(matches!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::ForeignCallbackBridge { bridge: found },
            kind: MirValidationErrorKind::InvalidForeignCallbackBridge { .. },
        }) if found == bridge
    ));
}

#[test]
fn callback_bridge_mode_identity_must_match_its_protocol_variant() {
    let (mut module, _, bridge) = callback_module();
    let record = module.foreign_callback_bridges[bridge]
        .application_record
        .clone();
    module.foreign_callback_bridges[bridge].application_record = CallbackApplicationRecord::new(
        record.application(),
        record.managed_adapter(),
        record.managed_signature().clone(),
        record.storage_abi(),
        CallbackMode::OneShot,
    );

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::ForeignCallbackBridge { bridge },
            kind: MirValidationErrorKind::InvalidForeignCallbackBridge {
                reason: "persistent callback mode does not match the protocol variant",
            },
        })
    );
}

#[test]
fn materialized_callback_adapter_uses_its_generated_callable_odr_member() {
    let (mut module, _, bridge) = callback_module();
    let callable_key =
        CallableApplicationKey::for_function(callback_owner(), CallableInstantiationOwner::NoOwner);
    let callable_application = PersistentCallableApplicationId::from_key(&callable_key).unwrap();
    let application_identity = callback_application_with_context(
        0,
        CallableMaterializationContext::Application(callable_application),
    );
    let application = application_identity.id();
    let generated = PersistentGeneratedCallableId::from_key(
        &GeneratedCallableKey::ForeignCallbackManagedAdapter { application },
    )
    .unwrap();
    let group = OdrGroupId::from_key(&SpecializationKey::Callable {
        application: callable_key,
    })
    .unwrap();
    let member = CborIdentityRecord::from_key(
        OdrMemberKey::new(
            group,
            OdrMemberRole::CallableBody,
            OdrMemberDiscriminator::GeneratedCallable(generated),
        )
        .unwrap(),
    )
    .unwrap();
    let managed_signature = module.foreign_callback_adapters
        [module.foreign_callback_bridges[bridge].adapter]
        .managed_signature;
    let exact_signature = exact_callback_signature();
    let adapter = module.foreign_callback_bridges[bridge].adapter;
    let adapter_function = module.foreign_callback_adapters[adapter].function;
    module.foreign_callback_adapters[adapter] = ForeignCallbackAdapter::checked(
        adapter_function,
        managed_signature,
        &module.function_types[managed_signature],
        application,
        &exact_signature,
        Some(member.clone()),
    )
    .unwrap();
    let subject = module.foreign_callback_adapters[adapter].signature_subject();
    assert!(matches!(
        subject,
        CallableSignatureSubject::Odr(found) if found.member() == member.id()
    ));
    module.foreign_callback_bridges[bridge].adapter = adapter;
    module.foreign_callback_bridges[bridge].application_identity = application_identity;
    module.foreign_callback_bridges[bridge].application_record = CallbackApplicationRecord::new(
        application,
        subject,
        exact_signature,
        ForeignCallbackStorageAbi::ClosureResultRootsThrowableToU32,
        CallbackMode::Reusable,
    );
    install_generated_callables(&mut module);

    assert!(module.validate().is_ok(), "{:?}", module.validate());
}

#[test]
fn callback_adapter_identity_must_match_its_application() {
    let (mut module, _, bridge) = callback_module();
    let managed_signature = module.foreign_callback_adapters
        [module.foreign_callback_bridges[bridge].adapter]
        .managed_signature;
    let adapter = module.foreign_callback_bridges[bridge].adapter;
    let adapter_function = module.foreign_callback_adapters[adapter].function;
    module.foreign_callback_adapters[adapter] = ForeignCallbackAdapter::checked(
        adapter_function,
        managed_signature,
        &module.function_types[managed_signature],
        callback_application(1).id(),
        &exact_callback_signature(),
        None,
    )
    .unwrap();
    module.foreign_callback_bridges[bridge].adapter = adapter;

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::ForeignCallbackBridge { bridge },
            kind: MirValidationErrorKind::InvalidForeignCallbackBridge {
                reason: "adapter persistent identity does not match the callback application",
            },
        })
    );
}

#[test]
fn callback_family_rejects_a_stale_code_pointer_signature_before_dumping() {
    let (mut module, family, _) = callback_module();
    let callback = module.foreign_callback_families[family].callback;
    let unknown = FunctionTypeId::from_raw(99.into());
    let StructRepresentation::Declared { fields, .. } =
        &mut module.structs[callback].representation
    else {
        panic!("callback fixture uses a declared struct")
    };
    fields[0].ty = Type::FunPtr(unknown);

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::ForeignCallbackFamily { family },
            kind: MirValidationErrorKind::InvalidForeignCallbackFamily {
                reason: "callback code-pointer signature reference is out of bounds",
            },
        })
    );
}

#[test]
fn callback_bridge_rejects_a_stale_adapter_function_before_dumping() {
    let (mut module, _, bridge) = callback_module();
    let adapter = module.foreign_callback_bridges[bridge].adapter;
    module.foreign_callback_adapters[adapter].function = FunctionId::from_raw(99.into());

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::ForeignCallbackBridge { bridge },
            kind: MirValidationErrorKind::InvalidForeignCallbackBridge {
                reason: "adapter function reference is out of bounds",
            },
        })
    );
}

#[test]
fn callback_bridge_rejects_a_stale_managed_signature_before_lowering() {
    let (mut module, _, bridge) = callback_module();
    let adapter = module.foreign_callback_bridges[bridge].adapter;
    module.foreign_callback_adapters[adapter].managed_signature =
        FunctionTypeId::from_raw(99.into());

    assert_eq!(
        module.validate(),
        Err(MirValidationError {
            location: MirValidationLocation::ForeignCallbackBridge { bridge },
            kind: MirValidationErrorKind::InvalidForeignCallbackBridge {
                reason: "adapter managed signature reference is out of bounds",
            },
        })
    );
}

#[test]
fn callback_failure_constructor_rejects_another_managed_payload_type() {
    let (mut module, family, _) = callback_module();
    let other = module.classes.alloc(ClassDef {
        link_stem: nominal_link_stem(),
        type_arguments: Vec::new(),
        modifier: ClassModifier::Final,
        name: "Other".to_string(),
        representation: ClassRepresentation::Declared {
            fields: Vec::new(),
            base_class: None,
        },
        interfaces: Vec::new(),
        vtable: Vec::new(),
        itables: Vec::new(),
    });
    register_test_exact_type(&mut module, &Type::Class(other));
    let result = module.foreign_callback_families[family].failure_result;
    let option = OptionCore::checked(&module.enums, result.some_payload(), result.none()).unwrap();
    assert!(ForeignCallbackFailureResult::checked(&module.enums, option, other).is_none());
}
