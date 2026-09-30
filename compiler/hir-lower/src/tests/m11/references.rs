use scoop_hir as hir;

use super::*;

#[test]
fn suspend_callable_reference_preserves_suspend_identity() {
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("waitForValue"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        suspend_fun("waitForValue", Vec::new()),
        fun(
            "main",
            vec![val_ty(
                "task",
                Some(ty_function(true, Vec::new(), ty_named("Unit"))),
                reference,
            )],
        ),
    ]))
    .expect("creating a suspend callable reference is synchronous");
    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("callable reference");
    assert!(module.function_types[reference.function_type].is_suspend);
}

#[test]
fn top_level_reference_is_a_distinct_typed_entity() {
    let signature = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("increment"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        fun_expr(
            "increment",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        fun(
            "main",
            vec![val_ty("operation", Some(signature), reference)],
        ),
    ]))
    .expect("top-level reference should lower");
    assert_eq!(module.callable_references.len(), 1);
    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("reference entity");
    assert_eq!(
        reference.definition_root,
        hir::LexicalDefinitionRoot::Function(module.entry())
    );
    assert_eq!(
        definition_path(&reference.definition_path),
        vec![(
            scoop_identity::StructuralDefinitionSiteRole::CallableConversion,
            0,
        )]
    );
    assert!(reference.captures.is_empty());
    let hir::CallableReferenceTarget::Named(hir::CallableTarget::Local(callable)) =
        &reference.target
    else {
        panic!("expected a top-level callable reference")
    };
    let target = module.callable_function(*callable);
    assert_eq!(module.functions[target].name, "increment");
}

#[test]
fn inapplicable_local_reference_layer_falls_through_to_top_level() {
    let signature = ty_function(false, vec![ty_named("String")], ty_named("String"));
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("choose"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        fun_expr(
            "choose",
            vec![],
            vec![("value", ty_named("String"))],
            Some(ty_named("String")),
            var("value"),
        ),
        fun(
            "main",
            vec![
                local_fun_sig(
                    "choose",
                    vec![],
                    vec![("value", ty_named("Int"))],
                    Some(ty_named("Int")),
                    vec![ret(Some(var("value")))],
                ),
                val_ty("operation", Some(signature), reference),
            ],
        ),
    ]))
    .expect("an inapplicable local reference layer must be skipped");

    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("reference entity");
    let hir::CallableReferenceTarget::Named(hir::CallableTarget::Local(callee)) = reference.target
    else {
        panic!("the top-level reference layer must win")
    };
    let function = module.callable_function(callee);
    assert_eq!(module.functions[function].name, "choose");
    assert_eq!(module.functions[function].params[0].ty, module.string);
}

#[test]
fn generic_top_level_reference_is_fixed_by_its_expected_type() {
    let signature = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("identity"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        fun_expr(
            "identity",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        ),
        fun(
            "main",
            vec![val_ty("operation", Some(signature), reference)],
        ),
    ]))
    .expect("the expected function type should fix the generic reference");

    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("reference entity");
    let hir::CallableReferenceTarget::Named(hir::CallableTarget::Local(hir::Callable::Generic(
        resolved,
    ))) = &reference.target
    else {
        panic!("expected a resolved generic reference")
    };
    assert_eq!(
        module.instantiations[*resolved].type_args,
        vec![int_type(&module)]
    );
}

#[test]
fn bound_reference_retains_receiver_and_resolved_member_identity() {
    let operation_ty = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: Some(Box::new(var("mapper"))),
        name: ident("map"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Mapper",
            vec![(false, "offset", ty_named("Int"))],
            None,
            vec![],
            vec![method_expr(
                "map",
                vec![("value", ty_named("Int"))],
                Some(ty_named("Int")),
                binary(ast::BinOp::Add, var("value"), var("offset")),
            )],
        ),
        fun(
            "main",
            vec![
                val("mapper", call("Mapper", vec![int_lit(2)])),
                val_ty("operation", Some(operation_ty), reference),
            ],
        ),
    ]))
    .expect("a bound member reference should lower");

    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("reference entity");
    let hir::CallableReferenceTarget::BoundMember { receiver, callee } = &reference.target else {
        panic!("expected a bound member target")
    };
    assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
    let target = module.callable_function(crate::tests::local_method_callable(&module, *callee));
    assert_eq!(module.functions[target].name, "Mapper.map");
    assert!(reference.captures.is_empty());
}

#[test]
fn extension_receiver_is_a_typed_this_parameter_and_direct_call_argument() {
    let module = lower_user(file(vec![
        extension_expr(
            ty_named("Int"),
            "bump",
            vec![],
            vec![("delta", ty_named("Int"))],
            Some(ty_named("Int")),
            binary(ast::BinOp::Add, this_expr(), var("delta")),
        ),
        fun(
            "main",
            vec![val(
                "result",
                method_call(int_lit(40), "bump", vec![int_lit(2)]),
            )],
        ),
    ]))
    .expect("extension call should lower");

    let extension = module
        .top_level
        .iter()
        .map(|id| &module.functions[*id])
        .find(|function| function.name == "bump")
        .expect("extension function");
    assert!(extension.method.is_none());
    assert_eq!(extension.params.len(), 2);
    assert_eq!(extension.params[0].name, "this");
    assert_eq!(extension.params[0].ty, int_type(&module));

    let hir::FunctionKind::User(main) = &module.functions[module.entry()].kind else {
        panic!("main body")
    };
    let init = local_init(main, "result");
    let hir::ExprKind::Call {
        callee: hir::CallableTarget::Local(callee),
        args,
        ..
    } = &init.kind
    else {
        panic!("extension invocation must be a direct call")
    };
    assert_eq!(
        module.functions[module.callable_function(*callee)].name,
        "bump"
    );
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].ty, int_type(&module));
}

#[test]
fn bound_extension_reference_has_a_distinct_direct_target() {
    let operation_ty = ty_function(false, vec![ty_named("String")], ty_named("String"));
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: Some(Box::new(str_lit("kept"))),
        name: ident("decorate"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        extension_expr(
            ty_named("String"),
            "decorate",
            vec![],
            vec![("suffix", ty_named("String"))],
            Some(ty_named("String")),
            binary(ast::BinOp::Add, this_expr(), var("suffix")),
        ),
        fun(
            "main",
            vec![val_ty("operation", Some(operation_ty), reference)],
        ),
    ]))
    .expect("bound extension reference should lower");

    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("reference entity");
    let hir::CallableReferenceTarget::BoundExtension {
        receiver,
        callee: hir::CallableTarget::Local(callee),
    } = &reference.target
    else {
        panic!("expected a bound extension target")
    };
    assert_eq!(receiver.ty, module.string);
    assert_eq!(
        module.functions[module.callable_function(*callee)].name,
        "decorate"
    );
}

#[test]
fn inapplicable_bound_member_reference_falls_through_to_extension() {
    let operation_ty = ty_function(false, vec![ty_named("String")], ty_named("String"));
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: Some(Box::new(var("host"))),
        name: ident("choose"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Host",
            vec![],
            None,
            vec![],
            vec![method_expr(
                "choose",
                vec![("value", ty_named("Int"))],
                Some(ty_named("Int")),
                var("value"),
            )],
        ),
        extension_expr(
            ty_named("Host"),
            "choose",
            vec![],
            vec![("value", ty_named("String"))],
            Some(ty_named("String")),
            var("value"),
        ),
        fun(
            "main",
            vec![
                val("host", struct_init("Host", vec![])),
                val_ty("operation", Some(operation_ty), reference),
            ],
        ),
    ]))
    .expect("an inapplicable member reference layer must be skipped");

    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("reference entity");
    let hir::CallableReferenceTarget::BoundExtension {
        callee: hir::CallableTarget::Local(callee),
        ..
    } = reference.target
    else {
        panic!("the extension reference layer must win")
    };
    let function = module.callable_function(callee);
    assert_eq!(module.functions[function].name, "choose");
    assert_eq!(module.functions[function].params.len(), 2);
}
