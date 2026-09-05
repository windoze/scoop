use scoop_hir as hir;

use super::*;

#[test]
fn function_types_are_canonical_and_keep_suspend_identity() {
    let ordinary = ty_function(false, vec![ty_named("Int")], ty_named("String"));
    let suspend = ty_function(true, vec![ty_named("Int")], ty_named("String"));
    let module = lower_user(file(vec![
        fun_sig(
            "use",
            vec![],
            vec![
                ("first", ordinary.clone()),
                ("second", ordinary),
                ("task", suspend),
            ],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]))
    .expect("function types should lower");

    let use_ = module
        .top_level
        .iter()
        .map(|id| &module.functions[*id])
        .find(|function| function.name == "use")
        .expect("use function");
    assert_eq!(use_.params[0].ty, use_.params[1].ty);
    assert_ne!(use_.params[0].ty, use_.params[2].ty);
    assert_eq!(
        hir::type_name(&module, use_.params[0].ty),
        "(Int) -> String"
    );
    assert_eq!(
        hir::type_name(&module, use_.params[2].ty),
        "suspend (Int) -> String"
    );
}

#[test]
fn function_types_substitute_type_parameters_recursively() {
    let generic = ty_function(false, vec![ty_named("T")], ty_named("T"));
    let module = lower_user(file(vec![
        fun_sig("consume", vec!["T"], vec![("op", generic)], None, vec![]),
        fun("main", vec![]),
    ]))
    .expect("generic function type should lower");

    let consume = module
        .top_level
        .iter()
        .map(|id| &module.functions[*id])
        .find(|function| function.name == "consume")
        .expect("consume function");
    let hir::Type::Function(id) = module.types[consume.params[0].ty] else {
        panic!("expected function type");
    };
    let signature = &module.function_types[id];
    assert!(matches!(
        module.types[signature.parameter_types[0]],
        hir::Type::Param(_)
    ));
    assert_eq!(signature.parameter_types[0], signature.return_type);
}

#[test]
fn function_types_are_reference_types_for_gc_constraints() {
    let mut lowerer = Lowerer::new();
    let int = lowerer.integer_type(hir::IntegerKind::SIGNED_32);
    let ty = lowerer.intern_function_type(false, vec![int], lowerer.string);
    assert!(lowerer.is_ref_ty(ty));
    assert!(!lowerer.is_value_ty(ty));
}

#[test]
fn callable_literal_inherits_the_enclosing_generic_namespace() {
    let result = ty_function(false, Vec::new(), ty_named("T"));
    let module = lower_user(file(vec![
        fun_expr(
            "make",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(result),
            lambda(Some(Vec::new()), var("value")),
        ),
        fun("main", vec![]),
    ]))
    .expect("a callable literal may retain the enclosing type parameter");

    let (_, lambda) = module.lambdas.iter().next().expect("lambda entity");
    assert_eq!(lambda.owner_type_param_count, 1);
    assert_eq!(module.functions[lambda.function].type_params()[0].name, "T");
    assert!(matches!(
        module.functions[lambda.function].genericity,
        hir::FunctionGenericity::Generic { .. }
    ));
    assert!(matches!(
        module.types[lambda.captures[0].ty],
        hir::Type::Param(_)
    ));
}

#[test]
fn function_variance_builds_an_explicit_typed_coercion() {
    let source_type = ty_function(false, vec![ty_named("Any")], ty_named("Int"));
    let target_type = ty_function(false, vec![ty_named("Int")], ty_named("Any"));
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("source"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        fun_expr(
            "source",
            Vec::new(),
            vec![("value", ty_named("Any"))],
            Some(ty_named("Int")),
            int_lit(42),
        ),
        fun(
            "main",
            vec![
                val_ty("exact", Some(source_type), reference),
                val_ty("widened", Some(target_type), var("exact")),
            ],
        ),
    ]))
    .expect("function parameters are contravariant and returns are covariant");

    let hir::FunctionKind::User(body) = &module.functions[module.entry].kind else {
        panic!("main body");
    };
    let hir::StatementKind::ValDecl { init, .. } = &body.statements[1].kind else {
        panic!("widened declaration");
    };
    let hir::ExprKind::FunctionCoercion {
        source,
        coercion,
        target_type,
    } = &init.kind
    else {
        panic!("variance must not be a pointer-only retype");
    };
    let conversion = &module.function_coercions[*coercion];
    assert_eq!(conversion.target, *target_type);
    assert!(matches!(source.kind, hir::ExprKind::Local(_)));
    assert_eq!(module.function_coercions.len(), 1);
}
