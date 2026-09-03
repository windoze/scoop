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
