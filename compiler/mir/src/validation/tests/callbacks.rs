use super::*;

use scoop_identity::{
    CallableMaterializationContext, CallbackApplicationKey, CallbackMode, CallbackParameterIndex,
    CallbackRegistrationKey, CanonicalIdentifier, ConeIdentity, CoreBuiltinNominal,
    DeclarationScope, DefinitionOwnerChain, Effect, LexicalCallableParent, PackagePath,
    PersistentCallbackApplicationId, PersistentFunctionId, SignatureCallableShape,
    SignatureTypeKey, SourceCAbiFunctionSignature, SourceCAbiReturn, SourceDeclarationKey,
    SourceDeclarationSite, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

fn unit_variant(name: &str) -> VariantDef {
    VariantDef {
        name: name.to_string(),
        gc_free: true,
        fields: Vec::new(),
    }
}

fn callback_application(ordinal: u32) -> PersistentCallbackApplicationId {
    let site = SourceDeclarationSite::new(
        ConeIdentity::CORE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new("callbackOwner").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
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
    let application = CallbackApplicationKey::new(
        &registration,
        CallableMaterializationContext::NoSubstitution,
    )
    .unwrap();
    PersistentCallbackApplicationId::from_key(&application).unwrap()
}

fn callback_module() -> (Module, ForeignCallbackFamilyId, ForeignCallbackBridgeId) {
    let (mut module, _) = module_with_variants(vec![unit_variant("Unused")]);
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
    let adapter = module
        .foreign_callback_adapters
        .alloc(ForeignCallbackAdapter {
            function: module.entry,
            managed_signature: signature,
        });
    let native_signature = module.function_types.alloc(FunctionType {
        is_suspend: false,
        parameter_types: vec![Type::Ptr(Box::new(Type::Unit))],
        return_type: Type::Unit,
    });
    let bridge = module
        .foreign_callback_bridges
        .alloc(ForeignCallbackBridge {
            application: callback_application(0),
            adapter,
            family,
            native_signature,
            context_index: 0,
            mode: modes.reusable(),
        });
    (module, family, bridge)
}

#[test]
fn callback_application_has_one_mir_bridge() {
    let (mut module, _, bridge) = callback_module();
    let source = &module.foreign_callback_bridges[bridge];
    let duplicate = ForeignCallbackBridge {
        application: source.application,
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
fn callback_family_metadata_revalidates_every_typed_role() {
    let (mut module, family, _) = callback_module();
    assert!(module.validate().is_ok());

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
    let result = module.foreign_callback_families[family].failure_result;
    let option = OptionCore::checked(&module.enums, result.some_payload(), result.none()).unwrap();
    assert!(ForeignCallbackFailureResult::checked(&module.enums, option, other).is_none());
}
