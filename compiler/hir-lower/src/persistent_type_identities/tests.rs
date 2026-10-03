use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::ExactTypeKey;

use crate::tests::{
    core_file, core_source_identity, field, file, fun_expr, generic_struct_decl,
    test_source_identity, tuple_lit, ty_function, ty_generic, ty_named, var,
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

fn lower_fixture_with_extra_type(extra_type: bool) -> hir::Output {
    let mut declarations = vec![
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        fun_expr(
            "open",
            vec!["T"],
            vec![("value", ty_generic("Box", vec![ty_named("T")]))],
            Some(ty_named("T")),
            field(var("value"), "value"),
        ),
        fun_expr(
            "closed",
            Vec::new(),
            vec![
                ("value", ty_generic("Box", vec![ty_named("Int")])),
                (
                    "mapping",
                    ty_function(false, vec![ty_named("Int")], ty_named("String")),
                ),
            ],
            Some(ty_generic("Box", vec![ty_named("Int")])),
            var("value"),
        ),
        fun_expr(
            "pair",
            Vec::new(),
            Vec::new(),
            Some(crate::tests::ty_tuple(vec![
                ty_named("Int"),
                ty_named("String"),
            ])),
            tuple_lit(vec![crate::tests::int_lit(1), crate::tests::str_lit("x")]),
        ),
        object("Registry"),
    ];
    if extra_type {
        declarations.insert(
            0,
            crate::tests::struct_decl("Unrelated", vec![("value", ty_named("String"))]),
        );
    }
    let source = file(declarations);
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(test_source_identity("src/types.scoop"), source),
        Vec::new(),
    ))
    .unwrap();
    let core = core_file();
    let input = crate::DefinedTestSources::try_new(
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
            display_locator: "/checkout/types.scoop",
            source_text: "",
        },
    )
    .unwrap();
    crate::lower_defined_for_test(
        scoop_identity::RequestedConeKind::Library,
        &input,
        crate::IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("the persistent type fixture lowers")
}

fn lower_fixture() -> hir::Output {
    lower_fixture_with_extra_type(false)
}

fn rebuild(module: &hir::Module) -> Result<hir::HirTypeIdentities, hir::HirTypeIdentityError> {
    hir::HirTypeIdentities::from_types(hir::HirTypeIdentityInputs {
        types: &module.types,
        function_types: &module.function_types,
        structs: &module.structs,
        struct_applications: &module.struct_applications,
        enums: &module.enums,
        loaded_enum_definitions: &module.loaded_enum_definitions,
        loaded_struct_definitions: &module.loaded_struct_definitions,
        loaded_class_definitions: &module.loaded_class_definitions,
        loaded_interface_definitions: &module.loaded_interface_definitions,
        enum_applications: &module.enum_applications,
        classes: &module.classes,
        class_applications: &module.class_applications,
        interfaces: &module.interfaces,
        interface_applications: &module.interface_applications,
        objects: &module.objects,
        core_types: hir::HirCoreTypeIdentityAuthority::Defined(
            &crate::tests::defined_export_core(module).fundamental_types,
        ),
        nominal_identities: &module.nominal_identities,
    })
}

#[test]
fn type_identity_is_total_and_distinguishes_exact_from_open_types() {
    let output = lower_fixture();
    let module = &output.export;
    for (ty, _) in module.types.iter() {
        match &module.type_identities[ty] {
            hir::HirTypeIdentity::Exact(record) => {
                assert_eq!(
                    record.id(),
                    scoop_identity::PersistentExactTypeId::from_key(record.key()).unwrap()
                );
            }
            hir::HirTypeIdentity::Open(open) => assert!(!open.parameters().is_empty()),
        }
    }

    let (_, boxed) = module
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Box")
        .expect("Box declaration");
    let open_box = module.struct_applications[boxed.self_application].canonical_type;
    let open_parameters = module.type_identities[open_box]
        .open()
        .expect("Box<T> is open")
        .parameters();
    assert_eq!(open_parameters, [boxed.type_params[0].id]);

    let closed = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "closed").then_some(function))
        .expect("closed function");
    let closed_box = closed.params[0].ty;
    assert!(matches!(
        module.type_identities[closed_box].exact().unwrap().key(),
        ExactTypeKey::NominalApplication { .. }
    ));
    assert!(matches!(
        module.type_identities[closed.params[1].ty]
            .exact()
            .unwrap()
            .key(),
        ExactTypeKey::Function { .. }
    ));

    let tuple = module
        .types
        .iter()
        .find_map(|(id, ty)| {
            matches!(ty, hir::Type::Tuple(elements) if elements.len() == 2).then_some(id)
        })
        .expect("closed tuple type");
    assert!(matches!(
        module.type_identities[tuple].exact().unwrap().key(),
        ExactTypeKey::Tuple(_)
    ));
}

#[test]
fn object_representation_uses_the_source_object_exact_identity() {
    let output = lower_fixture();
    let module = &output.export;
    let (object_id, object) = module
        .objects
        .iter()
        .find(|(_, declaration)| declaration.name == "Registry")
        .expect("Registry object");
    let ty = module.object_types[object.object_type].canonical_type;
    let source = module.nominal_identities[object_id]
        .concrete_type_id()
        .expect("object source identity");
    let backing = module.nominal_identities[object.backing_class]
        .concrete_type_id()
        .expect("generated backing identity");
    assert_eq!(
        module.type_identities[ty].exact().unwrap().key(),
        &ExactTypeKey::Nominal(source)
    );
    assert_ne!(source, backing);
}

#[test]
fn exact_type_identity_is_independent_of_type_arena_positions() {
    let identity = |output: &hir::Output| {
        let function = output
            .export
            .functions
            .iter()
            .find_map(|(_, function)| (function.name == "closed").then_some(function))
            .expect("closed function");
        output.export.type_identities[function.params[0].ty]
            .exact()
            .expect("Box<Int> is exact")
            .id()
    };
    let baseline = lower_fixture_with_extra_type(false);
    let shifted = lower_fixture_with_extra_type(true);
    assert_eq!(identity(&baseline), identity(&shifted));
}

#[test]
fn type_identity_rejects_noncanonical_and_duplicate_type_entries() {
    let output = lower_fixture();
    let mut changed = output.export.module().clone();
    let application = changed
        .struct_applications
        .iter()
        .find_map(|(id, application)| {
            matches!(
                changed.types[application.canonical_type],
                hir::Type::Struct(_)
            )
            .then_some(id)
        })
        .expect("a declared struct application");
    changed.struct_applications[application].canonical_type = changed.unit;
    assert!(matches!(
        rebuild(&changed),
        Err(hir::HirTypeIdentityError::InvalidApplication { .. })
    ));

    let mut changed = output.export.module().clone();
    changed.types.alloc(hir::Type::Unit);
    assert!(matches!(
        rebuild(&changed),
        Err(hir::HirTypeIdentityError::DuplicateExactIdentity { .. })
    ));

    let mut changed = output.export.module().clone();
    let first = changed
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::with_substitution_slot(
            999, 0,
        )));
    let second = changed
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::with_substitution_slot(
            999, 1,
        )));
    changed.types.alloc(hir::Type::Tuple(vec![first, second]));
    assert!(matches!(
        rebuild(&changed),
        Err(hir::HirTypeIdentityError::ConflictingBinderSlot { identity: 999, .. })
    ));

    let mut changed = output.export.module().clone();
    let hir::CoreProtocols::Defined(protocols) = &mut changed.core_protocols else {
        panic!("test Export HIR carries locally defined core protocols")
    };
    protocols.fundamental_types.boolean = hir::StructId::from_raw(999_u32.into());
    assert!(matches!(
        rebuild(&changed),
        Err(hir::HirTypeIdentityError::UnknownReference {
            relation: hir::HirTypeRelation::IntrinsicNominalOwner,
            target: 999,
            ..
        })
    ));
}
