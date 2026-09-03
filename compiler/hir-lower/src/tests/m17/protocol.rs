use super::super::*;

fn function_body<'module>(module: &'module hir::Module, name: &str) -> &'module hir::Body {
    let (_, function) = module
        .functions
        .iter()
        .find(|(_, function)| function.name == name)
        .expect("test function");
    let hir::FunctionKind::User(body) = &function.kind else {
        unreachable!("test function has a source body")
    };
    body
}

fn concrete_function_body<'module>(
    module: &'module hir::LocalConcreteHir,
    name: &str,
) -> &'module hir::concrete::Body {
    let (_, function) = module
        .functions
        .iter()
        .find(|(_, function)| function.name == name)
        .expect("concrete test function");
    let hir::concrete::FunctionKind::User(body) = &function.kind else {
        unreachable!("test function has a concrete source body")
    };
    body
}

fn call_with_span(name: &str, span: ast::Span) -> Expr {
    Expr::Call(ast::CallExpr {
        callee: ast::Ident {
            text: name.to_string(),
            span,
        },
        type_args: Vec::new(),
        args: Vec::new(),
        span,
    })
}

fn with_default(mut declaration: Decl, parameter: usize, expression: Expr) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        unreachable!("the test helper accepts a function declaration")
    };
    function.params[parameter].syntax = ast::ParameterSyntax::Default {
        expression,
        equals_span: sp(),
    };
    declaration
}

fn with_vararg(mut declaration: Decl, parameter: usize) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        unreachable!("the test helper accepts a function declaration")
    };
    function.params[parameter].syntax = ast::ParameterSyntax::Vararg {
        modifier_span: sp(),
        default: ast::VarargDefaultSyntax::EmptyWhenOmitted,
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

fn method_with_vararg(mut declaration: ast::FunctionDecl, parameter: usize) -> ast::FunctionDecl {
    declaration.params[parameter].syntax = ast::ParameterSyntax::Vararg {
        modifier_span: sp(),
        default: ast::VarargDefaultSyntax::EmptyWhenOmitted,
    };
    declaration
}

fn with_constructor_syntax(
    mut declaration: Decl,
    parameter: usize,
    syntax: ast::ParameterSyntax,
) -> Decl {
    let Decl::Class(class) = &mut declaration else {
        unreachable!("the test helper accepts a class declaration")
    };
    let ast::ClassConstructorDecl::Declared(parameters) = &mut class.constructor else {
        unreachable!("the test helper accepts an explicit primary constructor")
    };
    parameters[parameter].syntax = syntax;
    declaration
}

fn identity_lambda() -> Expr {
    Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(vec![ast::LambdaParam {
            target: pat_bind("value"),
            ty: Some(ty_named("T")),
            span: sp(),
        }]),
        body: block(vec![stmt(var("value"))]),
        span: sp(),
    }
}

#[test]
fn named_inputs_run_in_source_order_before_declaration_order_defaults() {
    let target = with_default(
        with_default(
            fun_sig(
                "target",
                vec![],
                vec![
                    ("x", ty_named("Int")),
                    ("y", ty_named("Int")),
                    ("z", ty_named("Int")),
                ],
                Some(ty_named("Int")),
                vec![ret(Some(var("x")))],
            ),
            0,
            call("defaultX", vec![]),
        ),
        2,
        call("defaultZ", vec![]),
    );
    let call = source_call(
        "target",
        vec![
            named_argument("z", call("explicitZ", vec![])),
            named_argument("y", call("explicitY", vec![])),
        ],
    );
    let module = lower_user(file(vec![
        fun_expr(
            "defaultX",
            vec![],
            vec![],
            Some(ty_named("Int")),
            int_lit(1),
        ),
        fun_expr(
            "defaultZ",
            vec![],
            vec![],
            Some(ty_named("Int")),
            int_lit(2),
        ),
        fun_expr(
            "explicitZ",
            vec![],
            vec![],
            Some(ty_named("Int")),
            int_lit(3),
        ),
        fun_expr(
            "explicitY",
            vec![],
            vec![],
            Some(ty_named("Int")),
            int_lit(4),
        ),
        target,
        fun("main", vec![val("result", call)]),
    ]))
    .expect("named/default call must lower");

    let dump = hir::dump(&module);
    let explicit_z = dump.find("Call explicitZ").expect("explicit z call");
    let explicit_y = dump.find("Call explicitY").expect("explicit y call");
    let default_x = dump.find("Call defaultX").expect("default x call");
    assert!(explicit_z < explicit_y && explicit_y < default_x, "{dump}");
    assert!(!dump.contains("Call defaultZ"), "{dump}");
}

#[test]
fn every_source_callable_and_constructor_uses_explicit_temporaries() {
    let output = lower_user_output(file(vec![
        fun_sig(
            "accept",
            vec![],
            vec![("value", ty_named("Int"))],
            None,
            vec![],
        ),
        struct_decl("Box", vec![("item", ty_named("Int"))]),
        class_decl(
            ast::ClassModifier::Final,
            "Holder",
            vec![(false, "content", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        enum_decl(
            "Choice",
            vec![],
            vec![variant_constructor(
                "Item",
                vec![("selected", ty_named("Int"), None)],
            )],
        ),
        fun(
            "main",
            vec![
                stmt(call("accept", vec![int_lit(1)])),
                val("boxed", call("Box", vec![int_lit(2)])),
                val("held", call("Holder", vec![int_lit(3)])),
                val(
                    "choice",
                    method_call(var("Choice"), "Item", vec![int_lit(4)]),
                ),
            ],
        ),
    ]))
    .expect("all source call shapes should share argument materialization");

    let body = function_body(&output.export, "main");
    let names = body
        .locals
        .iter()
        .map(|(_, local)| local.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names.iter().filter(|name| **name == "$argument.0").count(),
        4
    );
    for parameter in ["value", "item", "content", "selected"] {
        assert!(
            names.contains(&format!("$parameter.{parameter}").as_str()),
            "missing parameter temporary for {parameter}: {names:?}"
        );
    }
}

#[test]
fn callback_intrinsic_reads_named_constants_through_materialized_temporaries() {
    let native_signature = ty_function(
        false,
        vec![ty_named("Int"), ty_generic("Ptr", vec![ty_named("Unit")])],
        ty_named("Int"),
    );
    let callback = ast::Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(vec![ast::LambdaParam {
            target: pat_bind("value"),
            ty: Some(ty_named("Int")),
            span: sp(),
        }]),
        body: block(vec![stmt(var("value"))]),
        span: sp(),
    };
    let registration = typed_source_call(
        "foreignCallback",
        vec![native_signature],
        vec![
            named_argument(
                "mode",
                struct_init("ForeignCallbackMode.Reusable", Vec::new()),
            ),
            named_argument("callback", callback),
            named_argument("contextIndex", int_lit(1)),
        ],
    );
    let module = lower_user(file(vec![fun(
        "main",
        vec![unsafe_block(vec![val("registered", registration)])],
    )]))
    .expect("named callback arguments should survive explicit materialization");

    let (_, registration) = module
        .foreign_callback_registrations
        .iter()
        .next()
        .expect("one callback registration");
    assert_eq!(registration.context_index, 1);
    assert_eq!(registration.mode, hir::ForeignCallbackMode::Reusable);
    let hir::FunctionKind::User(main) = &module.functions[module.entry].kind else {
        panic!("main has a user body")
    };
    let dump = hir::dump(&module);
    assert!(dump.contains("Local $parameter.callback"), "{dump}");
    assert!(matches!(
        local_init(main, "registered").kind,
        hir::ExprKind::ForeignCallbackRegister { .. }
    ));
}

#[test]
fn local_default_uses_definition_binding_and_prior_parameter() {
    let mut local = local_fun_sig(
        "choose",
        vec![],
        vec![("first", ty_named("Int")), ("second", ty_named("Int"))],
        Some(ty_named("Int")),
        vec![ret(Some(var("second")))],
    );
    let ast::StatementKind::LocalFunction(function) = &mut local.kind else {
        unreachable!("local_fun_sig creates a local function")
    };
    function.params[0].syntax = ast::ParameterSyntax::Default {
        expression: var("base"),
        equals_span: sp(),
    };
    function.params[1].syntax = ast::ParameterSyntax::Default {
        expression: var("first"),
        equals_span: sp(),
    };
    let module = lower_user(file(vec![fun(
        "main",
        vec![
            val("base", int_lit(11)),
            local,
            val("result", call("choose", vec![])),
        ],
    )]))
    .expect("local defaults may capture lexical vals and reference prior parameters");
    let dump = hir::dump(&module);
    assert!(dump.contains("Local base : Int"), "{dump}");
    assert!(dump.contains("Local $parameter.first : Int"), "{dump}");
    assert!(
        dump.contains("LocalFunctionCall local0 $local.0.choose"),
        "{dump}"
    );
}

#[test]
fn a_default_does_not_infer_an_unconstrained_type_parameter() {
    let declaration = with_default(
        fun_sig(
            "empty",
            vec!["T"],
            vec![("values", ty_generic("Array", vec![ty_named("T")]))],
            Some(ty_generic("Array", vec![ty_named("T")])),
            vec![ret(Some(var("values")))],
        ),
        0,
        array_lit(vec![]),
    );
    let errors = lower_user(file(vec![
        declaration,
        fun("main", vec![val("values", call("empty", vec![]))]),
    ]))
    .expect_err("the omitted default contributes no inference constraint");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cannot infer a unique type argument for `T`")
    }));
}

#[test]
fn a_generic_default_callable_body_keeps_the_callee_application() {
    let function_type = ty_function(false, vec![ty_named("T")], ty_named("T"));
    let factory = with_default(
        fun_sig(
            "factory",
            vec!["T"],
            vec![("callback", function_type.clone())],
            Some(function_type),
            vec![ret(Some(var("callback")))],
        ),
        0,
        identity_lambda(),
    );
    let output = lower_user_output(file(vec![
        factory,
        fun(
            "main",
            vec![val(
                "callback",
                typed_call("factory", vec![ty_named("Int")], vec![]),
            )],
        ),
    ]))
    .expect("a default lambda from a generic callee must concretize at Int");
    assert!(output.local.functions.iter().any(|(_, function)| {
        function.name.starts_with("$lambda")
            && matches!(
                output.local.types[function.return_ty].kind,
                hir::concrete::TypeKind::Int
            )
    }));
}

#[test]
fn positional_vararg_assembles_fresh_array_but_named_vararg_keeps_identity() {
    let collect = with_vararg(
        fun_sig(
            "collect",
            vec![],
            vec![("values", ty_named("Int"))],
            Some(ty_generic("Array", vec![ty_named("Int")])),
            vec![ret(Some(var("values")))],
        ),
        0,
    );
    let mixed = source_call(
        "collect",
        vec![
            ast::CallArgument::positional(int_lit(1)),
            spread_argument(var("existing")),
        ],
    );
    let whole = source_call("collect", vec![named_argument("values", var("existing"))]);
    let module = lower_user(file(vec![
        collect,
        fun(
            "main",
            vec![
                val("existing", array_lit(vec![int_lit(2), int_lit(3)])),
                val("mixed", mixed),
                val("whole", whole),
                val("empty", call("collect", vec![])),
            ],
        ),
    ]))
    .expect("all vararg source forms must lower");
    let dump = hir::dump(&module);
    assert_eq!(
        dump.matches("ArrayAssembly : Array<Int>").count(),
        2,
        "{dump}"
    );
    assert!(
        dump.contains("CopyArray\n          Local $argument.1"),
        "{dump}"
    );
}

#[test]
fn base_constructor_delegation_uses_the_complete_source_protocol() {
    let base = with_constructor_syntax(
        with_constructor_syntax(
            with_constructor_syntax(
                class_decl(
                    ast::ClassModifier::Open,
                    "Base",
                    vec![
                        (false, "head", ty_named("Int")),
                        (false, "values", ty_named("Int")),
                        (false, "tail", ty_named("Int")),
                    ],
                    None,
                    vec![],
                    vec![],
                ),
                0,
                ast::ParameterSyntax::Default {
                    expression: int_lit(10),
                    equals_span: sp(),
                },
            ),
            1,
            ast::ParameterSyntax::Vararg {
                modifier_span: sp(),
                default: ast::VarargDefaultSyntax::EmptyWhenOmitted,
            },
        ),
        2,
        ast::ParameterSyntax::Default {
            expression: int_lit(30),
            equals_span: sp(),
        },
    );
    let mut derived = class_decl(
        ast::ClassModifier::Final,
        "Derived",
        vec![(false, "more", ty_generic("Array", vec![ty_named("Int")]))],
        Some(("Base", vec![])),
        vec![],
        vec![],
    );
    let Decl::Class(class) = &mut derived else {
        unreachable!()
    };
    class.base_class = Some((
        ty_named("Base"),
        vec![
            ast::CallArgument::positional(int_lit(1)),
            ast::CallArgument::positional(int_lit(2)),
            spread_argument(var("more")),
        ],
    ));
    let mut whole = class_decl(
        ast::ClassModifier::Final,
        "Whole",
        vec![
            (false, "more", ty_generic("Array", vec![ty_named("Int")])),
            (false, "last", ty_named("Int")),
        ],
        Some(("Base", vec![])),
        vec![],
        vec![],
    );
    let Decl::Class(class) = &mut whole else {
        unreachable!()
    };
    class.base_class = Some((
        ty_named("Base"),
        vec![
            named_argument("values", var("more")),
            named_argument("tail", var("last")),
        ],
    ));

    let module = lower_user(file(vec![base, derived, whole, fun("main", vec![])]))
        .expect("base delegation must support defaults and positional vararg parts");
    let (_, derived) = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Derived")
        .expect("derived class");
    let (_, delegation) = derived.base_class.as_ref().expect("base delegation");
    assert_eq!(delegation.args.len(), 3);
    assert!(delegation.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::ValDecl {
            init: hir::Expr {
                kind: hir::ExprKind::ArrayAssembly(_),
                ..
            },
            ..
        }
    )));
    assert!(delegation.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::ValDecl {
            init: hir::Expr {
                kind: hir::ExprKind::IntLiteral(30),
                ..
            },
            ..
        }
    )));

    let (_, whole) = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Whole")
        .expect("whole-array derived class");
    let (_, delegation) = whole.base_class.as_ref().expect("named base delegation");
    assert!(!delegation.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::ValDecl {
            init: hir::Expr {
                kind: hir::ExprKind::ArrayAssembly(_),
                ..
            },
            ..
        }
    )));
    assert!(delegation.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::ValDecl {
            init: hir::Expr {
                kind: hir::ExprKind::IntLiteral(10),
                ..
            },
            ..
        }
    )));
}

#[test]
fn msc_prefers_fewer_defaults_then_a_non_vararg_declaration() {
    let defaulted = with_default(
        fun_expr(
            "pick",
            vec![],
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("defaulted"),
        ),
        1,
        int_lit(0),
    );
    let vararg = with_vararg(
        fun_expr(
            "gather",
            vec![],
            vec![("values", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("vararg"),
        ),
        0,
    );
    let module = lower_user(file(vec![
        fun_expr(
            "pick",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            int_lit(1),
        ),
        defaulted,
        fun_expr(
            "gather",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            int_lit(2),
        ),
        vararg,
        fun(
            "main",
            vec![
                val("a", call("pick", vec![int_lit(1)])),
                val("b", call("gather", vec![int_lit(1)])),
            ],
        ),
    ]))
    .expect("MSC additions choose a unique candidate");
    let main = function_body(&module, "main");
    let result_types = main
        .locals
        .iter()
        .filter(|(_, local)| local.name == "a" || local.name == "b")
        .map(|(_, local)| hir::type_name(&module, local.ty))
        .collect::<Vec<_>>();
    assert_eq!(result_types, vec!["Int", "Int"]);
}

#[test]
fn override_inherits_default_but_each_static_view_keeps_its_parameter_name() {
    let interface_method = method_with_default(
        bodyless_method(
            false,
            "draw",
            vec![("width", ty_named("Int"))],
            Some(ty_named("Int")),
        ),
        0,
        int_lit(10),
    );
    let implementation = override_method_expr(
        "draw",
        vec![("size", ty_named("Int"))],
        Some(ty_named("Int")),
        var("size"),
    );
    let module = lower_user(file(vec![
        interface_decl("Drawable", vec![interface_method]),
        class_decl(
            ast::ClassModifier::Final,
            "Shape",
            vec![],
            None,
            vec!["Drawable"],
            vec![implementation],
        ),
        fun(
            "main",
            vec![
                val("shape", call("Shape", vec![])),
                val("defaulted", method_call(var("shape"), "draw", vec![])),
                val(
                    "concreteName",
                    source_method_call(
                        var("shape"),
                        "draw",
                        vec![named_argument("size", int_lit(1))],
                    ),
                ),
                val_ty(
                    "drawable",
                    Some(ty_named("Drawable")),
                    call("Shape", vec![]),
                ),
                val(
                    "interfaceName",
                    source_method_call(
                        var("drawable"),
                        "draw",
                        vec![named_argument("width", int_lit(2))],
                    ),
                ),
            ],
        ),
    ]))
    .expect("an override inherits one default source while keeping its own parameter name");
    let dump = hir::dump(&module);
    assert!(dump.contains("IntLiteral 10 : Int"), "{dump}");
    assert!(dump.contains("MethodCall Shape.draw"), "{dump}");
    assert!(dump.contains("MethodCall Drawable.draw"), "{dump}");
}

#[test]
fn inherited_default_carries_the_parent_to_child_type_relation() {
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
    let mut child_declaration = class_decl(
        ast::ClassModifier::Final,
        "Child",
        vec![],
        None,
        vec![],
        vec![implementation],
    );
    let Decl::Class(child) = &mut child_declaration else {
        unreachable!()
    };
    child.type_params = vec![type_param("T")];
    child.interfaces = vec![ty_generic(
        "Parent",
        vec![ty_generic("Array", vec![ty_named("T")])],
    )];

    let output = lower_user_output(file(vec![
        generic_interface_decl(
            "Parent",
            vec![(ast::Variance::Invariant, "P")],
            vec![inherited],
        ),
        child_declaration,
        fun(
            "main",
            vec![
                val("child", typed_call("Child", vec![ty_named("Int")], vec![])),
                val(
                    "selected",
                    method_call(var("child"), "choose", vec![array_lit(vec![int_lit(1)])]),
                ),
            ],
        ),
    ]))
    .expect("the inherited template parameter must map through Array<T>");

    let main = function_body(&output.export, "main");
    let selected = main
        .locals
        .iter()
        .find(|(_, local)| local.name == "selected")
        .map(|(_, local)| local)
        .expect("selected local");
    assert_eq!(hir::type_name(&output.export, selected.ty), "Array<Int>");
}

#[test]
fn concrete_default_origins_keep_definition_and_outermost_evaluation_sites() {
    let inner_definition = ast::Span::new(10, 12);
    let nested_call_definition = ast::Span::new(20, 25);
    let outer_call_site = ast::Span::new(100, 107);
    let inner = with_default(
        fun_sig(
            "inner",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![ret(Some(var("value")))],
        ),
        0,
        Expr::IntLiteral {
            value: 41,
            span: inner_definition,
        },
    );
    let outer = with_default(
        fun_sig(
            "outer",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![ret(Some(var("value")))],
        ),
        0,
        call_with_span("inner", nested_call_definition),
    );
    let output = lower_user_output(file(vec![
        inner,
        outer,
        fun(
            "main",
            vec![val("result", call_with_span("outer", outer_call_site))],
        ),
    ]))
    .expect("nested defaults must preserve complete expression origins");

    assert!(
        output
            .export
            .export_default_exprs
            .iter()
            .all(|(_, template)| {
                matches!(template.value.origin, hir::ExpressionOrigin::Definition(_))
                    && template.statements.iter().all(|statement| {
                        !matches!(
                            &statement.kind,
                            hir::StatementKind::ValDecl {
                                init: hir::Expr {
                                    origin: hir::ExpressionOrigin::Instantiated(_),
                                    ..
                                },
                                ..
                            }
                        )
                    })
            })
    );

    let main = concrete_function_body(&output.local, "main");
    let origin = main
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::ValDecl {
                init:
                    hir::concrete::Expr {
                        kind: hir::concrete::ExprKind::IntLiteral(41),
                        origin,
                        ..
                    },
                ..
            } => Some(*origin),
            _ => None,
        })
        .expect("instantiated inner default literal");
    assert_eq!(origin.definition.span, inner_definition);
    assert_eq!(origin.evaluation.span, outer_call_site);
}

#[test]
fn a_lambda_body_establishes_its_own_default_evaluation_boundary() {
    let inner_definition = ast::Span::new(10, 12);
    let lambda_call_site = ast::Span::new(40, 47);
    let factory_call_site = ast::Span::new(100, 109);
    let inner = with_default(
        fun_sig(
            "inner",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![ret(Some(var("value")))],
        ),
        0,
        Expr::IntLiteral {
            value: 42,
            span: inner_definition,
        },
    );
    let callback_type = ty_function(false, vec![], ty_named("Int"));
    let factory = with_default(
        fun_sig(
            "factory",
            vec![],
            vec![("callback", callback_type.clone())],
            Some(callback_type),
            vec![ret(Some(var("callback")))],
        ),
        0,
        Expr::Lambda {
            id: ast::LambdaId(0),
            is_suspend: false,
            parameters: Some(Vec::new()),
            body: block(vec![stmt(call_with_span("inner", lambda_call_site))]),
            span: ast::Span::new(30, 50),
        },
    );
    let output = lower_user_output(file(vec![
        inner,
        factory,
        fun(
            "main",
            vec![val(
                "callback",
                call_with_span("factory", factory_call_site),
            )],
        ),
    ]))
    .expect("default-instantiated lambdas must retain their body origin boundary");

    let (_, lambda) = output
        .local
        .functions
        .iter()
        .find(|(_, function)| function.name.starts_with("$lambda"))
        .expect("lambda body");
    let hir::concrete::FunctionKind::User(body) = &lambda.kind else {
        unreachable!()
    };
    let origin = body
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::ValDecl {
                init:
                    hir::concrete::Expr {
                        kind: hir::concrete::ExprKind::IntLiteral(42),
                        origin,
                        ..
                    },
                ..
            } => Some(*origin),
            _ => None,
        })
        .expect("inner default in lambda body");
    assert_eq!(origin.definition.span, inner_definition);
    assert_eq!(origin.evaluation.span, lambda_call_site);
    assert_ne!(origin.evaluation.span, factory_call_site);
}

#[test]
fn current_source_location_reads_the_concrete_evaluation_origin() {
    let source = concat!(
        "fun trace(location: SourceLocation = getCurrentSourceLocation()): SourceLocation = location\n",
        "fun main() {\n",
        "    val direct = getCurrentSourceLocation()\n",
        "    val forwarded = trace()\n",
        "}\n",
    );
    let span_at = |needle: &str, after: usize| {
        let offset = source[after..].find(needle).expect("source marker") + after;
        ast::Span::new(offset as u32, (offset + needle.len()) as u32)
    };
    let default_span = span_at("getCurrentSourceLocation()", 0);
    let direct_span = span_at("getCurrentSourceLocation()", default_span.end as usize);
    let forwarded_span = span_at("trace()", direct_span.end as usize);
    let trace = with_default(
        fun_expr(
            "trace",
            vec![],
            vec![("location", ty_named("SourceLocation"))],
            Some(ty_named("SourceLocation")),
            var("location"),
        ),
        0,
        call_with_span("getCurrentSourceLocation", default_span),
    );
    let user = file(vec![
        trace,
        fun(
            "main",
            vec![
                val(
                    "direct",
                    call_with_span("getCurrentSourceLocation", direct_span),
                ),
                val("forwarded", call_with_span("trace", forwarded_span)),
            ],
        ),
    ]);
    let core = core_file();
    let core_provider = hir::IntrinsicProviderId::from_raw(0);
    let user_provider = hir::IntrinsicProviderId::from_raw(1);
    let output = lower_compilation_unit(
        &CompilationUnit {
            core: vec![ProviderSource {
                source: &core,
                provider: core_provider,
                name: "scoop.core",
                source_text: "",
            }],
            user: ProviderSource {
                source: &user,
                provider: user_provider,
                name: "app.scoop",
                source_text: source,
            },
        },
        IntrinsicDeclarationPolicy::CoreOnly,
    )
    .expect("source locations should fold during ordinary concretization");

    let main = concrete_function_body(&output.local, "main");
    let locations: Vec<_> = main
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::ValDecl { init, .. } => location_fields(init),
            _ => None,
        })
        .collect();
    assert!(
        locations.contains(&("app.scoop", 3, 18, "main", "")),
        "locations: {locations:?}"
    );
    assert!(
        locations.contains(&("app.scoop", 4, 21, "main", "")),
        "locations: {locations:?}"
    );
}

fn location_fields(expr: &hir::concrete::Expr) -> Option<(&str, i64, i64, &str, &str)> {
    let hir::concrete::ExprKind::StructInit { args, .. } = &expr.kind else {
        return None;
    };
    let [file, line, column, function_name, type_name] = args.as_slice() else {
        return None;
    };
    let (
        hir::concrete::ExprKind::StringLiteral(file),
        hir::concrete::ExprKind::IntLiteral(line),
        hir::concrete::ExprKind::IntLiteral(column),
        hir::concrete::ExprKind::StringLiteral(function_name),
        hir::concrete::ExprKind::StringLiteral(type_name),
    ) = (
        &file.kind,
        &line.kind,
        &column.kind,
        &function_name.kind,
        &type_name.kind,
    )
    else {
        return None;
    };
    Some((file, *line, *column, function_name, type_name))
}

#[test]
fn source_location_context_distinguishes_member_generic_and_suspend_bodies() {
    let member = class_decl(
        ast::ClassModifier::Final,
        "Recorder",
        vec![],
        None,
        vec![],
        vec![method_expr(
            "locate",
            vec![],
            Some(ty_named("SourceLocation")),
            call("getCurrentSourceLocation", vec![]),
        )],
    );
    let mut suspend_location = fun_sig(
        "suspendLocation",
        vec![],
        vec![],
        Some(ty_named("SourceLocation")),
        vec![ret(Some(call("getCurrentSourceLocation", vec![])))],
    );
    let Decl::Function(suspend_location_decl) = &mut suspend_location else {
        unreachable!()
    };
    suspend_location_decl.is_suspend = true;
    let output = lower_user_output(file(vec![
        member,
        fun_expr(
            "genericLocation",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("SourceLocation")),
            call("getCurrentSourceLocation", vec![]),
        ),
        suspend_location,
        fun(
            "main",
            vec![
                val("recorder", call("Recorder", vec![])),
                val("member", method_call(var("recorder"), "locate", vec![])),
                val("generic", call("genericLocation", vec![int_lit(1)])),
            ],
        ),
    ]))
    .expect("source location context is available in every callable shape");

    let context_for = |name: &str| {
        let (_, function) = output
            .local
            .functions
            .iter()
            .find(|(_, function)| function.name == name)
            .unwrap_or_else(|| panic!("missing concrete function `{name}`"));
        let hir::concrete::FunctionKind::User(body) = &function.kind else {
            unreachable!()
        };
        body.statements
            .iter()
            .find_map(|statement| match &statement.kind {
                hir::concrete::StatementKind::Return { value: Some(value) } => {
                    location_fields(value)
                }
                hir::concrete::StatementKind::ValDecl { init, .. } => location_fields(init),
                _ => None,
            })
            .map(|(_, _, _, function, ty)| (function.to_string(), ty.to_string()))
            .unwrap_or_else(|| panic!("missing source location in `{name}`"))
    };
    assert_eq!(
        context_for("Recorder.locate"),
        ("locate".to_string(), "Recorder".to_string())
    );
    assert_eq!(
        context_for("genericLocation"),
        ("genericLocation".to_string(), String::new())
    );
    assert_eq!(
        context_for("suspendLocation"),
        ("suspendLocation".to_string(), String::new())
    );
}

#[test]
fn exported_defaults_carry_kind_typed_references_and_access_witnesses() {
    let consume = with_default(
        fun_sig(
            "consume",
            vec![],
            vec![("token", ty_named("Token"))],
            Some(ty_named("Int")),
            vec![ret(Some(field(var("token"), "value")))],
        ),
        0,
        struct_init("Token", vec![call("produce", vec![])]),
    );
    let output = lower_user_output(file(vec![
        struct_decl("Token", vec![("value", ty_named("Int"))]),
        fun_expr("produce", vec![], vec![], Some(ty_named("Int")), int_lit(7)),
        consume,
        fun("main", vec![stmt(call("consume", vec![]))]),
    ]))
    .expect("an exported default must normalize every direct dependency");

    let (consume, _) = output
        .export
        .functions
        .iter()
        .find(|(_, function)| function.name == "consume")
        .expect("consume function");
    let interface = output
        .export
        .source_parameter_interfaces
        .iter()
        .find(|interface| interface.owner == hir::ExportParameterOwner::Function(consume))
        .expect("consume source interface");
    let hir::ExportParameterCalling::Default { source, .. } = interface.parameters[0].calling
    else {
        unreachable!()
    };
    let template = &output.export.export_default_exprs
        [output.export.export_default_sources[source].expression];
    assert_eq!(template.references.callables.len(), 1);
    assert_eq!(template.references.constructors.len(), 1);
    assert!(!template.references.types.is_empty());
    let expected_owner = hir::ExportParameterOwner::Function(consume);
    assert!(template.references.callables.iter().all(|reference| {
        reference.witness.owner == expected_owner
            && reference.witness.coverage == hir::ExportDefaultAccessCoverage::ConeWide
    }));
    assert!(template.references.constructors.iter().all(|reference| {
        reference.witness.owner == expected_owner
            && reference.witness.coverage == hir::ExportDefaultAccessCoverage::ConeWide
    }));
    assert!(
        template
            .references
            .types
            .iter()
            .all(|reference| reference.witness.owner == expected_owner)
    );
}

#[test]
fn override_rejects_new_defaults_vararg_mismatch_and_conflicting_sources() {
    let explicit_default = method_with_default(
        override_method_expr(
            "draw",
            vec![("width", ty_named("Int"))],
            Some(ty_named("Int")),
            var("width"),
        ),
        0,
        int_lit(20),
    );
    let errors = lower_user(file(vec![
        interface_decl(
            "Drawable",
            vec![method_with_default(
                bodyless_method(
                    false,
                    "draw",
                    vec![("width", ty_named("Int"))],
                    Some(ty_named("Int")),
                ),
                0,
                int_lit(10),
            )],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Shape",
            vec![],
            None,
            vec!["Drawable"],
            vec![explicit_default],
        ),
        fun("main", vec![]),
    ]))
    .expect_err("overrides cannot replace a default expression");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message == "override function `draw` cannot declare a new default expression"
    }));

    let mismatch = override_method_expr(
        "add",
        vec![("values", ty_generic("Array", vec![ty_named("Int")]))],
        None,
        unit_lit(),
    );
    let errors = lower_user(file(vec![
        interface_decl(
            "Collector",
            vec![method_with_vararg(
                bodyless_method(false, "add", vec![("values", ty_named("Int"))], None),
                0,
            )],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "CollectorImpl",
            vec![],
            None,
            vec!["Collector"],
            vec![mismatch],
        ),
        fun("main", vec![]),
    ]))
    .expect_err("an override must preserve the vararg source shape");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("must have the same `vararg` shape")
    }));

    let interface = |name: &str, value| {
        interface_decl(
            name,
            vec![method_with_default(
                bodyless_method(
                    false,
                    "value",
                    vec![("input", ty_named("Int"))],
                    Some(ty_named("Int")),
                ),
                0,
                int_lit(value),
            )],
        )
    };
    let errors = lower_user(file(vec![
        interface("Left", 1),
        interface("Right", 2),
        class_decl(
            ast::ClassModifier::Final,
            "Both",
            vec![],
            None,
            vec!["Left", "Right"],
            vec![override_method_expr(
                "value",
                vec![("input", ty_named("Int"))],
                Some(ty_named("Int")),
                var("input"),
            )],
        ),
        fun("main", vec![]),
    ]))
    .expect_err("unrelated inherited default sources are ambiguous");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("inherits conflicting default expressions")
    }));
}
