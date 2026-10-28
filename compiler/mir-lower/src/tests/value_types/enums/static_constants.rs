use super::*;

#[test]
fn static_enum_constants_preserve_exact_refs_and_payloads_through_hir_and_mir() {
    let mut h = Harness::new();
    let option_int = h.option(h.int);
    let option_string = h.option(h.string);
    let int_application = h.enum_application_of(option_int);
    let string_application = h.enum_application_of(option_string);
    let none = hir::EnumVariantRef::checked(&h.enums, h.option_enum, 1)
        .expect("test core Option has None");
    let int_none = hir::AppliedEnumVariantRef::checked(
        &h.enums,
        &h.enum_applications,
        &h.nominal_identities(),
        int_application,
        none,
    )
    .expect("None belongs to Option<Int>");
    let string_none = hir::AppliedEnumVariantRef::checked(
        &h.enums,
        &h.enum_applications,
        &h.nominal_identities(),
        string_application,
        none,
    )
    .expect("None belongs to Option<String>");
    assert_ne!(int_none, string_none);
    let some = hir::EnumVariantRef::checked(&h.enums, h.option_enum, 0).unwrap();
    let int_some = hir::AppliedEnumVariantRef::checked(
        &h.enums,
        &h.enum_applications,
        &h.nominal_identities(),
        int_application,
        some,
    )
    .unwrap();
    let string_some = hir::AppliedEnumVariantRef::checked(
        &h.enums,
        &h.enum_applications,
        &h.nominal_identities(),
        string_application,
        some,
    )
    .unwrap();
    let nested = h.option(option_string);
    let nested_some = hir::AppliedEnumVariantRef::checked(
        &h.enums,
        &h.enum_applications,
        &h.nominal_identities(),
        h.enum_application_of(nested),
        some,
    )
    .unwrap();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let executable = h.finish(main);
    let entry = executable.entry();
    let mut export = executable.into_module();
    let nested_payload = hir::HirConstantImage::Enum {
        variant: hir::EnumVariantApplication {
            owner: string_some.application(),
            variant: export.enum_member_identities[string_some.declaration()].id(),
        },
        fields: vec![hir::HirConstantImage::String("nested".into())],
    };
    for (name, ty, variant, fields) in [
        ("noneInt", option_int, int_none, Vec::new()),
        ("noneString", option_string, string_none, Vec::new()),
        (
            "someInt",
            option_int,
            int_some,
            vec![hir::HirConstantImage::Integer(
                hir::HirIntegerConstant::Signed32(42),
            )],
        ),
        (
            "someString",
            option_string,
            string_some,
            vec![hir::HirConstantImage::String("payload".into())],
        ),
        ("nested", nested, nested_some, vec![nested_payload]),
    ] {
        let global = hir::GlobalId::from_raw((export.globals.len() as u32).into());
        let property = hir::PropertyId::from_raw((export.properties.len() as u32).into());
        let getter = export.property_getters.alloc(hir::PropertyGetter {
            access: hir::DeclarationAccess::public(),
            implementation: hir::PropertyAccessorImplementation::Storage,
            attributes: hir::FunctionAttributes::default(),
            span: SPAN,
        });
        let variant = hir::EnumVariantApplication {
            owner: variant.application(),
            variant: export.enum_member_identities[variant.declaration()].id(),
        };
        let actual_global = export.globals.alloc(hir::Global {
            name: name.to_string(),
            property,
            ty,
            mutable: false,
            storage: hir::GlobalStorage::Managed {
                state: hir::HirStaticInitialState::EncodedStaticValue {
                    payload: hir::HirConstantImage::Enum { variant, fields },
                },
            },
            span: SPAN,
        });
        assert_eq!(actual_global, global);
        let actual_property = export.properties.alloc(hir::Property {
            owner: hir::PropertyOwner::TopLevel,
            name: name.to_string(),
            access: hir::DeclarationAccess::public(),
            modifier: hir::MethodModifier::Final,
            is_override: false,
            overrides: Vec::new(),
            ty,
            capability: hir::PropertyCapability::ReadOnly { getter },
            representation: hir::PropertyRepresentation::Stored(hir::StoredProperty {
                backing: hir::PropertyBacking::TopLevelGlobal {
                    storage: global,
                    initialization: hir::TopLevelInitialization::Image,
                },
            }),
            span: SPAN,
        });
        assert_eq!(actual_property, property);
    }
    export.property_identities = crate::tests::harness_nominals::test_property_identities(
        &export.properties,
        &export.extension_properties,
        &export.nominal_identities,
    );
    export.property_accessor_identities =
        crate::tests::harness_nominals::test_property_accessor_identities(
            &export.properties,
            &export.property_identities,
            &export.property_getters,
            &export.property_setters,
        );

    let export = executable_output(export, entry);
    let concrete =
        scoop_hir_lower::concretize_output(&export).expect("concrete type applications are valid");
    let concrete_refs = ["noneInt", "noneString"].map(|name| {
        let global = concrete
            .globals
            .iter()
            .find_map(|(_, global)| (global.name == name).then_some(global))
            .expect("concrete global");
        let hir::concrete::TypeKind::Enum(enum_id) = &concrete.types[global.ty].kind else {
            panic!("constant keeps its exact concrete enum type")
        };
        let hir::concrete::GlobalStorage::Managed {
            state:
                hir::concrete::HirStaticInitialState::EncodedStaticValue {
                    payload: hir::concrete::HirConstantImage::Enum { variant, .. },
                },
        } = &global.storage
        else {
            panic!("constant keeps its checked concrete unit variant")
        };
        assert_eq!(variant.enumeration(), *enum_id);
        assert_eq!(
            concrete.enums[*enum_id].variants[variant.variant().into_raw() as usize].name,
            "None"
        );
        let hir::concrete::PropertyStorageOwner::Backing(owner) = global.storage_owner else {
            panic!("stored global keeps its persistent property backing owner")
        };
        (*variant, owner)
    });
    assert_ne!(concrete_refs[0].0, concrete_refs[1].0);

    let module = crate::lower(&concrete)
        .expect("test LocalConcrete HIR carries locally defined core protocols");
    let mir_refs = ["noneInt", "noneString"].map(|name| {
        let global = module
            .globals
            .iter()
            .find_map(|(_, global)| (global.name == name).then_some(global))
            .expect("MIR global");
        let mir::Type::Enum(enum_id, arguments) = &global.ty else {
            panic!("constant keeps its exact MIR enum type")
        };
        assert_eq!(arguments, &module.enums[*enum_id].type_arguments);
        let mir::GlobalStorage::Managed {
            initial_state:
                mir::MirStaticInitialState::EncodedStaticValue {
                    payload: mir::MirConstantImage::Enum { variant, .. },
                },
        } = &global.storage
        else {
            panic!("constant keeps its checked MIR unit variant")
        };
        assert_eq!(variant.enum_id(), *enum_id);
        assert_eq!(variant.definition(&module.enums).unwrap().name, "None");
        *variant
    });
    for (name, (_, owner)) in ["noneInt", "noneString"].into_iter().zip(concrete_refs) {
        let global = module
            .globals
            .iter()
            .find_map(|(_, global)| (global.name == name).then_some(global))
            .expect("MIR global");
        assert_eq!(
            global.storage_owner,
            mir::StaticStorageOwner::PropertyBacking(owner)
        );
    }
    assert_ne!(mir_refs[0], mir_refs[1]);
    assert_eq!(
        module.enums[mir_refs[0].enum_id()].type_arguments,
        vec![mir::Type::Integer(mir::IntegerKind::SIGNED_32)]
    );
    assert_eq!(
        module.enums[mir_refs[1].enum_id()].type_arguments,
        vec![mir::Type::String]
    );
    let payload = |name: &str| {
        let global = module
            .globals
            .iter()
            .find_map(|(_, global)| (global.name == name).then_some(global))
            .unwrap();
        let mir::GlobalStorage::Managed {
            initial_state: mir::MirStaticInitialState::EncodedStaticValue { payload },
        } = &global.storage
        else {
            panic!("static image")
        };
        payload
    };
    assert!(
        matches!(payload("someInt"), mir::MirConstantImage::Enum { fields, .. } if fields == &[mir::MirConstantImage::Integer(mir::MirIntegerConstant::Signed32(42))])
    );
    let mir::MirConstantImage::Enum {
        variant: outer,
        fields,
    } = payload("nested")
    else {
        panic!("nested enum")
    };
    let [
        mir::MirConstantImage::Enum {
            variant: inner,
            fields,
        },
    ] = fields.as_slice()
    else {
        panic!("nested payload")
    };
    assert_ne!(outer.enum_id(), inner.enum_id());
    let [mir::MirConstantImage::String(string)] = fields.as_slice() else {
        panic!("nested string")
    };
    assert_eq!(module.strings[*string].value, "nested");
    let mir::MirConstantImage::Enum { fields, .. } = payload("someString") else {
        panic!("string enum")
    };
    let [mir::MirConstantImage::String(string)] = fields.as_slice() else {
        panic!("string payload")
    };
    assert_eq!(module.strings[*string].value, "payload");
}
