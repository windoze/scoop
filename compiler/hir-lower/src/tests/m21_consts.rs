use super::*;

fn const_property(name: &str, ty: TypeRef, expression: Expr) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body: ast::PropertyBodySyntax::Const(Box::new(expression)),
        span: sp(),
    }
}

fn ordinary_property(name: &str, expression: Expr) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: vec![ast::Annotation {
            name: ident("Global"),
            args: Vec::new(),
            span: sp(),
        }],
        mutable: true,
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(expression),
            accessors: ast::AccessorSyntax::default(),
        },
        ..const_property(name, ty_named("Int"), int_lit(0))
    }
}

fn user_function<'module>(module: &'module hir::Module, name: &str) -> &'module hir::Function {
    module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == name).then_some(function))
        .unwrap_or_else(|| panic!("function `{name}` must exist"))
}

fn function_result(function: &hir::Function) -> &hir::Expr {
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("test function must have a user body")
    };
    return_value(&body.statements)
}

#[test]
fn const_properties_fold_forward_references_and_disappear_at_read_sites() {
    let mut answer = const_property(
        "answer",
        ty_named("Int"),
        binary(BinOp::Add, var("base"), int_lit(2)),
    );
    answer.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    };
    let source = file(vec![
        Decl::Global(answer),
        Decl::Global(const_property("base", ty_named("Int"), int_lit(40))),
        Decl::Global(const_property(
            "label",
            ty_named("String"),
            binary(BinOp::Add, str_lit("forty"), str_lit("-two")),
        )),
        Decl::Global(const_property(
            "valid",
            ty_named("Boolean"),
            binary(BinOp::Eq, var("answer"), int_lit(42)),
        )),
        fun_expr(
            "readAnswer",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            var("answer"),
        ),
        fun_expr(
            "readLabel",
            Vec::new(),
            Vec::new(),
            Some(ty_named("String")),
            var("label"),
        ),
        fun_expr(
            "readValid",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Boolean")),
            var("valid"),
        ),
        fun("main", Vec::new()),
    ]);
    let module = lower_user(source).expect("valid const definitions must lower");

    let (_, answer) = module
        .properties
        .iter()
        .find(|(_, property)| property.name == "answer")
        .expect("answer const property");
    assert!(matches!(
        answer.representation,
        hir::PropertyRepresentation::Const {
            value: hir::ConstPropertyValue::Integer(42)
        }
    ));
    assert_eq!(
        module.property_getters[answer.capability.getter()].implementation,
        hir::PropertyAccessorImplementation::Constant
    );
    assert!(
        module
            .public_surface
            .properties
            .iter()
            .any(|property| { module.properties[*property].name == "answer" })
    );
    assert!(
        module.globals.iter().all(|(_, global)| !matches!(
            global.name.as_str(),
            "answer" | "base" | "label" | "valid"
        ))
    );

    assert!(matches!(
        function_result(user_function(&module, "readAnswer")).kind,
        hir::ExprKind::IntLiteral(42)
    ));
    assert!(matches!(
        &function_result(user_function(&module, "readLabel")).kind,
        hir::ExprKind::StringLiteral(value) if value == "forty-two"
    ));
    assert!(matches!(
        function_result(user_function(&module, "readValid")).kind,
        hir::ExprKind::BoolLiteral(true)
    ));
}

#[test]
fn const_rejects_invalid_declarations_and_non_constant_expressions() {
    let mut mutable = const_property("mutable", ty_named("Int"), int_lit(1));
    mutable.mutable = true;
    let source = file(vec![
        Decl::Global(mutable),
        Decl::Global(const_property("unit", ty_named("Unit"), unit_lit())),
        Decl::Global(ordinary_property("ordinary", int_lit(3))),
        Decl::Global(const_property(
            "ordinaryRead",
            ty_named("Int"),
            var("ordinary"),
        )),
        Decl::Global(const_property(
            "called",
            ty_named("Int"),
            call("helper", Vec::new()),
        )),
        fun_expr(
            "helper",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        ),
        fun("main", Vec::new()),
    ]);
    let errors = lower_user(source).expect_err("invalid const definitions must fail");
    let messages = errors
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();
    assert!(messages.contains(&"const property must be a `val`".to_string()));
    assert!(messages.contains(&"const property `unit` has unsupported type Unit".to_string()));
    assert!(
        messages.contains(
            &"const initializer may only reference const properties; `ordinary` is not const"
                .to_string()
        )
    );
    assert!(messages.contains(
        &"const initializer must contain only literals, const references, and built-in primitive operators"
            .to_string()
    ));
}

#[test]
fn const_dependency_cycle_is_detected_through_short_circuit_rhs() {
    let source = file(vec![
        Decl::Global(const_property(
            "first",
            ty_named("Boolean"),
            binary(BinOp::Or, bool_lit(true), var("second")),
        )),
        Decl::Global(const_property(
            "second",
            ty_named("Boolean"),
            binary(BinOp::And, bool_lit(false), var("first")),
        )),
        fun("main", Vec::new()),
    ]);
    let errors = lower_user(source).expect_err("const dependency cycles must fail");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message == "const dependency cycle: first -> second -> first"
    }));
}

#[test]
fn const_visibility_is_checked_before_folding() {
    let mut secret = const_property("secret", ty_named("Int"), int_lit(41));
    secret.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    let source = file(vec![
        Decl::Global(const_property(
            "answer",
            ty_named("Int"),
            binary(BinOp::Add, var("secret"), int_lit(1)),
        )),
        fun("main", Vec::new()),
    ]);
    let errors = lower(&[core_file(), file(vec![Decl::Global(secret)]), source])
        .expect_err("file-private const must not be visible from another file");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message == "const property `secret` is not accessible here"
    }));
}
