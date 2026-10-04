use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::InitializationUnitKey;

use crate::tests::{
    call, class_decl, core_file, core_source_identity, file, fun_expr, ident, int_lit, method_full,
    sp, test_source_identity, ty_named, var,
};

fn delegate_operator(mut function: ast::FunctionDecl) -> ast::FunctionDecl {
    function.operator = Some(ast::OperatorModifier { span: sp() });
    function
}

fn delegate_method(
    name: &str,
    params: Vec<(&str, ast::TypeRef)>,
    return_ty: ast::TypeRef,
    body: ast::Expr,
) -> ast::FunctionDecl {
    delegate_operator(method_full(
        false,
        false,
        name,
        params,
        Some(return_ty),
        ast::FunctionBody::Expr(Box::new(body)),
    ))
}

fn runtime_property(name: &str) -> ast::Decl {
    ast::Decl::Global(ast::PropertyDecl {
        context_parameters: Vec::new(),
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(call("seed", Vec::new())),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    })
}

fn extension_delegate() -> ast::Decl {
    ast::Decl::Global(ast::PropertyDecl {
        context_parameters: Vec::new(),
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: Some(ty_named("Int")),
        type_params: Vec::new(),
        where_clause: None,
        name: ident("shared"),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Delegated {
            expression: Box::new(call("GlobalDelegate", vec![int_lit(2)])),
            by_span: sp(),
        },
        span: sp(),
    })
}

fn object(name: &str) -> ast::Decl {
    ast::Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        supertypes: Vec::new(),
        members: Vec::new(),
        span: sp(),
    })
}

fn host_with_companion() -> ast::Decl {
    let ast::Decl::Class(mut host) = class_decl(
        ast::ClassModifier::Final,
        "Host",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    ) else {
        unreachable!("class builder returns a class")
    };
    host.members.push(ast::ClassMember::Companion(Box::new(
        ast::CompanionObjectDecl {
            annotations: Vec::new(),
            visibility: ast::VisibilitySyntax::Omitted,
            name: ast::CompanionNameSyntax::Named(ident("Factory")),
            supertypes: Vec::new(),
            members: Vec::new(),
            span: sp(),
        },
    )));
    ast::Decl::Class(host)
}

fn lower_fixture() -> hir::Output {
    let source = file(vec![
        fun_expr(
            "seed",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        ),
        runtime_property("runtimeValue"),
        class_decl(
            ast::ClassModifier::Final,
            "GlobalDelegate",
            vec![(true, "value", ty_named("Int"))],
            None,
            Vec::new(),
            vec![
                delegate_method(
                    "provideDelegate",
                    Vec::new(),
                    ty_named("GlobalDelegate"),
                    var("this"),
                ),
                delegate_method(
                    "getValue",
                    vec![("thisRef", ty_named("Int"))],
                    ty_named("Int"),
                    var("value"),
                ),
            ],
        ),
        extension_delegate(),
        object("Registry"),
        host_with_companion(),
    ]);
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(test_source_identity("src/main.scoop"), source),
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
            display_locator: "/checkout/main.scoop",
            source_text: "",
        },
    )
    .unwrap();
    crate::lower_defined_for_test(
        scoop_identity::RequestedConeKind::Library,
        &input,
        crate::IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("the initialization-unit fixture lowers")
}

fn rebuild(
    module: &hir::Module,
) -> Result<hir::HirInitializationUnitIdentities, hir::HirInitializationUnitIdentityError> {
    hir::HirInitializationUnitIdentities::from_declarations(
        &module.initialization_units,
        &module.initialization_failure_roots,
        &module.functions,
        &module.globals,
        &module.objects,
        &module.companion_relations,
        &module.singleton_values,
        &module.singleton_published_roots,
        &module.properties,
        &module.delegate_storages,
        &module.generic_delegate_templates,
        &module.nominal_identities,
        &module.property_identities,
    )
}

#[test]
fn initialization_identities_cover_each_typed_declaration_kind() {
    let output = lower_fixture();
    let module = &output.export;
    let mut ordinary = false;
    let mut extension = false;
    let mut standalone = false;
    let mut companion = false;

    for (unit_id, unit) in module.initialization_units.iter() {
        let key = module.initialization_unit_identities[unit_id].key();
        match (unit.kind, key) {
            (
                hir::InitializationUnitKind::EagerTopLevel { property, .. },
                InitializationUnitKey::TopLevelProperty(id),
            ) if module.properties[property].name == "runtimeValue" => {
                assert_eq!(
                    module.property_identities[property].ordinary_id(),
                    Some(*id)
                );
                ordinary = true;
            }
            (
                hir::InitializationUnitKind::EagerTopLevel { property, .. },
                InitializationUnitKey::ExtensionProperty(id),
            ) if module.properties[property].name == "shared" => {
                assert_eq!(
                    module.property_identities[property].extension_id(),
                    Some(*id)
                );
                extension = true;
            }
            (
                hir::InitializationUnitKind::LazySingleton { value, .. },
                InitializationUnitKey::Object(id),
            ) if module.objects[module.singleton_values[value].declaration].name == "Registry" => {
                let object = module.singleton_values[value].declaration;
                assert_eq!(
                    module.nominal_identities[object].concrete_type_id(),
                    Some(*id)
                );
                standalone = true;
            }
            (
                hir::InitializationUnitKind::LazySingleton { value, .. },
                InitializationUnitKey::Companion(id),
            ) if module.objects[module.singleton_values[value].declaration].name == "Factory" => {
                let object = module.singleton_values[value].declaration;
                assert_eq!(
                    module.nominal_identities[object].concrete_type_id(),
                    Some(*id)
                );
                companion = true;
            }
            _ => {}
        }
    }

    assert!(ordinary && extension && standalone && companion);
}

#[test]
fn initialization_identity_ignores_display_names_and_dependency_order() {
    let output = lower_fixture();
    let expected = output
        .export
        .initialization_units
        .iter()
        .map(|(id, _)| output.export.initialization_unit_identities[id].id())
        .collect::<Vec<_>>();
    let mut changed = output.export.module().clone();
    for (_, unit) in changed.initialization_units.iter_mut() {
        unit.display_name = "changed display name".to_string();
        unit.dependencies.reverse();
    }
    let rebuilt = rebuild(&changed).expect("display-only changes preserve identity");
    let actual = changed
        .initialization_units
        .iter()
        .map(|(id, _)| rebuilt[id].id())
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

#[test]
fn initialization_identity_rejects_inconsistent_unit_relations() {
    let output = lower_fixture();
    let mut changed = output.export.module().clone();
    let (unit, _) = changed
        .initialization_units
        .iter()
        .find(|(_, unit)| matches!(unit.kind, hir::InitializationUnitKind::LazySingleton { .. }))
        .expect("the fixture has a singleton unit");
    changed.initialization_units[unit].schedule = hir::InitializationSchedule::EagerStartup;
    assert!(matches!(
        rebuild(&changed),
        Err(hir::HirInitializationUnitIdentityError::Schedule { .. })
    ));

    let mut changed = output.export.module().clone();
    let units = changed
        .initialization_units
        .iter()
        .map(|(id, _)| id)
        .take(2)
        .collect::<Vec<_>>();
    let duplicate = changed.initialization_units[units[0]].failure_root;
    changed.initialization_units[units[1]].failure_root = duplicate;
    assert!(matches!(
        rebuild(&changed),
        Err(
            hir::HirInitializationUnitIdentityError::DuplicateReference {
                relation: hir::InitializationRelation::FailureRoot,
                ..
            }
        )
    ));
}
