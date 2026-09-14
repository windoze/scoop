use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{DefinitionOwnerAtom, DuplicateSignatureKey, GeneratedCallableKey};

use crate::tests::{
    class_decl, core_file, core_source_identity, file, generic_struct_decl, test_source_identity,
    ty_generic, ty_named,
};

fn object(name: &str) -> ast::Decl {
    ast::Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: crate::tests::ident(name),
        supertypes: Vec::new(),
        members: Vec::new(),
        span: crate::tests::sp(),
    })
}

fn lower_fixture(extra_type: bool) -> hir::Output {
    let mut declarations = vec![
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        class_decl(
            ast::ClassModifier::Final,
            "Record",
            vec![(false, "value", ty_named("Int"))],
            None,
            Vec::new(),
            Vec::new(),
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Consumer",
            vec![(false, "value", ty_generic("Box", vec![ty_named("Int")]))],
            None,
            Vec::new(),
            Vec::new(),
        ),
        object("Registry"),
    ];
    if extra_type {
        declarations.insert(
            0,
            crate::tests::struct_decl("Unrelated", vec![("text", ty_named("String"))]),
        );
    }
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(
            test_source_identity("src/constructors.scoop"),
            file(declarations),
        ),
        Vec::new(),
    ))
    .unwrap();
    let core = core_file();
    let input = crate::LegacyCombinedSources::try_new(
        vec![crate::ProviderSource {
            source: &core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| crate::CurrentSourceDetails {
            display_locator: "/checkout/constructors.scoop",
            source_text: "",
        },
    )
    .unwrap();
    crate::lower_combined_sources(
        scoop_identity::RequestedConeKind::Library,
        &input,
        crate::IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("the constructor identity fixture lowers")
}

fn rebuild(
    module: &hir::Module,
) -> Result<hir::HirConstructorIdentities, hir::HirConstructorIdentityError> {
    let structs = module
        .struct_constructors
        .iter()
        .map(|(id, _)| module.constructor_identities[id].clone())
        .collect();
    let classes = module
        .class_constructors
        .iter()
        .map(|(id, _)| module.constructor_identities[id].clone())
        .collect();
    hir::HirConstructorIdentities::checked(
        hir::HirConstructorIdentityInputs {
            type_inputs: hir::HirTypeIdentityInputs {
                types: &module.types,
                function_types: &module.function_types,
                structs: &module.structs,
                struct_applications: &module.struct_applications,
                enums: &module.enums,
                enum_applications: &module.enum_applications,
                classes: &module.classes,
                class_applications: &module.class_applications,
                interfaces: &module.interfaces,
                interface_applications: &module.interface_applications,
                objects: &module.objects,
                intrinsic_core: &module.intrinsic_type_core,
                nominal_identities: &module.nominal_identities,
            },
            struct_constructors: &module.struct_constructors,
            class_constructors: &module.class_constructors,
            class_constructor_applications: &module.class_constructor_applications,
        },
        structs,
        classes,
    )
}

#[test]
fn constructor_identities_cover_source_generic_object_and_generated_adapter_cases() {
    let output = lower_fixture(false);
    let module = &output.export;

    let (boxed_id, boxed) = module
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Box")
        .unwrap();
    let boxed_constructor = boxed.constructors[0];
    let record = &module.constructor_identities[boxed_constructor];
    let DuplicateSignatureKey::Constructor { parameters } = record.key().duplicate_signature()
    else {
        panic!("Box constructor has a constructor signature")
    };
    assert!(matches!(
        parameters.as_slice(),
        [scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }]
    ));
    assert_eq!(
        record.key().owners().owners().last(),
        Some(
            &module.nominal_identities[boxed_id]
                .source()
                .unwrap()
                .definition_owner()
        )
    );

    let (object_id, object) = module
        .objects
        .iter()
        .find(|(_, declaration)| declaration.name == "Registry")
        .unwrap();
    let object_constructor = module.classes[object.backing_class].constructors[0];
    let object_record = module.constructor_identities[object_constructor]
        .source_record()
        .unwrap();
    assert_eq!(
        object_record.key().owners().owners().last(),
        Some(&DefinitionOwnerAtom::Type(
            module.nominal_identities[object_id]
                .concrete_type_id()
                .unwrap()
        ))
    );

    let adapters = module
        .class_constructors
        .iter()
        .filter_map(|(id, constructor)| match constructor.identity_kind {
            hir::ClassConstructorIdentityKind::ZeroArgumentAdapter { source } => Some((id, source)),
            hir::ClassConstructorIdentityKind::Source => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(adapters.len(), 1);
    let (adapter, source) = adapters[0];
    let identity = &module.constructor_identities[adapter];
    assert!(matches!(
        identity.generated_record().unwrap().key(),
        GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor }
            if *constructor == module.constructor_identities[source]
                .source_record()
                .unwrap()
                .id()
    ));
}

#[test]
fn constructor_identity_ignores_unrelated_arena_insertions() {
    let identity = |output: &hir::Output| {
        let constructor = output
            .export
            .structs
            .iter()
            .find(|(_, declaration)| declaration.name == "Box")
            .unwrap()
            .1
            .constructors[0];
        output.export.constructor_identities[constructor].id()
    };
    assert_eq!(
        identity(&lower_fixture(false)),
        identity(&lower_fixture(true))
    );
}

#[test]
fn constructor_identity_relation_rejects_adapter_shape_corruption() {
    let output = lower_fixture(false);
    let mut module = output.export.module().clone();
    let adapter = module
        .class_constructors
        .iter()
        .find_map(|(id, constructor)| {
            matches!(
                constructor.identity_kind,
                hir::ClassConstructorIdentityKind::ZeroArgumentAdapter { .. }
            )
            .then_some(id)
        })
        .unwrap();
    let definition = module.class_constructors[adapter].origin;
    module.class_constructors[adapter]
        .parameters
        .push(hir::ConstructorParameter {
            id: hir::ConstructorParamId::from_raw(999_u32),
            binding: hir::BindingId::from_raw(999_u32),
            definition,
            name: "invalid".to_string(),
            ty: module.unit,
        });
    assert!(matches!(
        rebuild(&module),
        Err(hir::HirConstructorIdentityError::AdapterShape { .. })
    ));
}

#[test]
fn constructor_identity_relation_rejects_invalid_parameter_type_application() {
    let output = lower_fixture(false);
    let mut module = output.export.module().clone();
    let consumer = module
        .classes
        .iter()
        .find(|(_, declaration)| declaration.name == "Consumer")
        .unwrap()
        .1;
    let parameter_type = module.class_constructors[consumer.constructors[0]].parameters[0].ty;
    let hir::Type::Struct(application) = module.types[parameter_type] else {
        panic!("Consumer parameter has an applied struct type")
    };
    module.struct_applications[application].canonical_type = module.unit;

    assert!(matches!(
        rebuild(&module),
        Err(hir::HirConstructorIdentityError::SourceDerivation {
            error: hir::HirSourceConstructorIdentityError::InvalidSignatureType(
                hir::HirSignatureTypeMappingError::InvalidApplication(_)
            ),
            ..
        })
    ));
}

#[test]
fn constructor_identity_relation_rejects_invalid_parameter_type_arity() {
    let output = lower_fixture(false);
    let mut module = output.export.module().clone();
    let consumer = module
        .classes
        .iter()
        .find(|(_, declaration)| declaration.name == "Consumer")
        .unwrap()
        .1;
    let parameter_type = module.class_constructors[consumer.constructors[0]].parameters[0].ty;
    let hir::Type::Struct(application) = module.types[parameter_type] else {
        panic!("Consumer parameter has an applied struct type")
    };
    module.struct_applications[application].arguments.clear();

    assert!(matches!(
        rebuild(&module),
        Err(hir::HirConstructorIdentityError::SourceDerivation {
            error: hir::HirSourceConstructorIdentityError::InvalidSignatureType(
                hir::HirSignatureTypeMappingError::NominalArity {
                    expected: 1,
                    actual: 0
                }
            ),
            ..
        })
    ));
}
