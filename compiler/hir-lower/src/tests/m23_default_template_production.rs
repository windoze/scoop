use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};
use scoop_wire::{WirePath, encode};

use super::*;

fn public_declarations(declarations: Vec<Decl>) -> Vec<Decl> {
    let mut source = file(declarations);
    make_core_public(&mut source);
    source.declarations
}

fn with_default(mut declaration: Decl, parameter: usize, expression: Expr) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        panic!("the test helper accepts a function declaration")
    };
    function.params[parameter].syntax = ast::ParameterSyntax::Default {
        expression,
        equals_span: sp(),
    };
    declaration
}

fn method_with_default(
    mut declaration: ast::FunctionDecl,
    parameter: usize,
    expression: Expr,
) -> ast::FunctionDecl {
    declaration.params[parameter].syntax = ast::ParameterSyntax::Default {
        expression,
        equals_span: sp(),
    };
    declaration
}

fn function_id(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

fn callable_id(module: &hir::Module, function: hir::FunctionId) -> CallableTemplateOrigin {
    match &module.function_identities[function] {
        hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
            CallableTemplateOrigin::Function(record.id())
        }
        hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(record)) => {
            CallableTemplateOrigin::GenericFunction(record.id())
        }
        identity => panic!("expected source callable identity, found {identity:?}"),
    }
}

fn lambda_at(tail: Expr, span: ast::Span) -> Expr {
    Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: None,
        body: ast::Block {
            statements: vec![Statement {
                kind: StatementKind::Expr(tail),
                span,
            }],
            span,
        },
        span,
    }
}

#[test]
fn producer_projects_a_typed_default_and_exact_reference_closure() {
    let declaration = with_default(
        fun_expr(
            "projectedDefaultBody",
            vec!["T"],
            vec![("required", ty_named("T")), ("optional", ty_named("T"))],
            Some(ty_named("T")),
            var("optional"),
        ),
        1,
        var("required"),
    );
    let module = lower_core_with_additional_declarations(public_declarations(vec![declaration]));
    let owner = callable_id(&module, function_id(&module, "projectedDefaultBody"));
    let key = hir::ExportDefaultTemplateKeyV1::new(owner, 1);
    let templates = hir::CanonicalExportDefaultTemplatesV1::from_export_hir(&module)
        .expect("the declaration-bound HIR default must project canonically");

    let template = templates.get(key).expect("the default key must be present");
    assert_eq!(
        template.result(),
        &SignatureTypeKey::Binder { depth: 0, index: 0 }
    );
    assert_eq!(
        template.type_parameters().arguments(),
        &[SignatureTypeKey::Binder { depth: 0, index: 0 }]
    );
    assert!(matches!(
        template.body().value().kind(),
        hir::DefaultExpressionKindV1::Local(scoop_identity::LocalValueSelector::Parameter {
            declaration_index: 0,
            ..
        })
    ));

    template
        .validate_reference_closure(&WirePath::root())
        .expect("the producer must emit the exact six-domain reference closure");
    assert!(
        !encode(&templates.index_locals().unwrap())
            .unwrap()
            .is_empty(),
        "projected local selectors must be indexable for canonical wire output"
    );
}

#[test]
fn producer_closes_callable_constructor_and_owner_type_references() {
    let consume = with_default(
        fun_sig(
            "consumeProjectedToken",
            Vec::new(),
            vec![("token", ty_named("ProjectedToken"))],
            Some(ty_named("Int")),
            vec![ret(Some(field(var("token"), "value")))],
        ),
        0,
        struct_init(
            "ProjectedToken",
            vec![typed_call(
                "produceProjectedValue",
                vec![ty_named("String")],
                Vec::new(),
            )],
        ),
    );
    let module = lower_core_with_additional_declarations(public_declarations(vec![
        struct_decl("ProjectedToken", vec![("value", ty_named("Int"))]),
        fun_expr(
            "produceProjectedValue",
            vec!["T"],
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(7),
        ),
        consume,
    ]));
    let owner = callable_id(&module, function_id(&module, "consumeProjectedToken"));
    let templates = hir::CanonicalExportDefaultTemplatesV1::from_export_hir(&module).unwrap();
    let template = templates
        .get(hir::ExportDefaultTemplateKeyV1::new(owner, 0))
        .unwrap();

    assert_eq!(template.references().callables().len(), 1);
    assert_eq!(template.references().constructors().len(), 1);
    template
        .validate_reference_closure(&WirePath::root())
        .expect("constructor owner and callable shape types belong to the exact closure");
}

#[test]
fn inherited_default_keeps_provider_identity_and_composed_binder_mapping() {
    let inherited = method_with_default(
        bodyless_method(
            false,
            "choose",
            vec![("seed", ty_named("P")), ("value", ty_named("P"))],
            Some(ty_named("P")),
        ),
        1,
        var("seed"),
    );
    let implementation = override_method_expr(
        "choose",
        vec![
            ("seed", ty_generic("Array", vec![ty_named("T")])),
            ("value", ty_generic("Array", vec![ty_named("T")])),
        ],
        Some(ty_generic("Array", vec![ty_named("T")])),
        var("value"),
    );
    let mut child = class_decl(
        ast::ClassModifier::Final,
        "ProjectedChild",
        Vec::new(),
        None,
        Vec::new(),
        vec![implementation],
    );
    let Decl::Class(child_declaration) = &mut child else {
        unreachable!()
    };
    child_declaration.type_params = vec![type_param("T")];
    child_declaration.supertypes = vec![bare_supertype(ty_generic(
        "ProjectedParent",
        vec![ty_generic("Array", vec![ty_named("T")])],
    ))];
    let module = lower_core_with_additional_declarations(public_declarations(vec![
        generic_interface_decl("ProjectedParent", vec!["P"], vec![inherited]),
        child,
    ]));
    let parent = callable_id(&module, function_id(&module, "ProjectedParent.choose"));
    let child = callable_id(&module, function_id(&module, "ProjectedChild.choose"));
    let templates = hir::CanonicalExportDefaultTemplatesV1::from_export_hir(&module).unwrap();
    let template = templates
        .get(hir::ExportDefaultTemplateKeyV1::new(child, 1))
        .expect("the override must publish its inherited default under the current owner");

    assert_eq!(template.definition_root().declaration(), parent);
    assert!(matches!(
        &template.type_parameters().arguments()[0],
        SignatureTypeKey::NominalApplication { arguments, .. }
            if arguments.as_slice()
                == [SignatureTypeKey::Binder { depth: 0, index: 0 }]
    ));

    template
        .validate_reference_closure(&WirePath::root())
        .expect("an inherited template retains the original typed reference closure");
}

#[test]
fn producer_closes_nested_callable_capture_and_function_types() {
    let function_type = ty_function(false, Vec::new(), ty_named("String"));
    let capture_span = ast::Span::new(30, 34);
    let lambda_span = ast::Span::new(20, 40);
    let default = lambda_at(Expr::Var(ident_at("seed", capture_span)), lambda_span);
    let declaration = with_default(
        fun_expr(
            "projectedClosure",
            Vec::new(),
            vec![
                ("seed", ty_named("String")),
                ("callback", function_type.clone()),
            ],
            Some(function_type),
            var("callback"),
        ),
        1,
        default,
    );
    let module = lower_core_with_additional_declarations(public_declarations(vec![declaration]));
    let owner = callable_id(&module, function_id(&module, "projectedClosure"));
    let templates = hir::CanonicalExportDefaultTemplatesV1::from_export_hir(&module).unwrap();
    let template = templates
        .get(hir::ExportDefaultTemplateKeyV1::new(owner, 1))
        .unwrap();

    let hir::DefaultExpressionKindV1::Lambda(lambda) = template.body().value().kind() else {
        panic!("the default body must retain its lambda descriptor")
    };
    assert_eq!(lambda.captures().len(), 1);
    assert!(template.references().callables().iter().any(|reference| {
        matches!(
            reference.target(),
            hir::ExportDefaultCallableTargetV1::Lambda { .. }
        )
    }));
    template
        .validate_reference_closure(&WirePath::root())
        .expect("nested callable metadata must participate in the exact reference closure");
}

#[test]
fn producer_rejects_a_constructor_only_expression_in_a_default() {
    let declaration = with_default(
        fun_expr(
            "invalidProjectedDefault",
            Vec::new(),
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        0,
        int_lit(1),
    );
    let mut module =
        lower_core_with_additional_declarations(public_declarations(vec![declaration]))
            .into_module();
    let function = function_id(&module, "invalidProjectedDefault");
    let interface = module
        .source_parameter_interfaces
        .iter()
        .find(|interface| interface.owner == hir::ExportParameterOwner::Function(function))
        .unwrap();
    let hir::ExportParameterCalling::Default { source, .. } = interface.parameters[0].calling
    else {
        unreachable!()
    };
    let expression = module.export_default_sources[source].expression;
    module.export_default_exprs[expression].value.kind =
        hir::ExprKind::ConstructorParam(hir::ConstructorParamId::from_raw(0));

    assert!(matches!(
        hir::CanonicalExportDefaultTemplatesV1::from_export_hir(&module),
        Err(hir::DefaultTemplateProductionError::Template { source, .. })
            if matches!(
                source.as_ref(),
                hir::DefaultTemplateEnvelopeProjectionError::Body(
                    hir::DefaultBodyProjectionError::UnsupportedExpression(
                        "constructor-parameter"
                    )
                )
            )
    ));
}
