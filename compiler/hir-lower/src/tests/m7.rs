//! M7 tests: function overloading (docs/milestone7/DESIGN.md) —
//! declaration rules (1.1), the layered + most-specific resolution
//! algorithm (1.2), and the `print` / `println` migration to ordinary
//! core overloads (section 2).

use super::*;
use ast::ClassModifier::*;

/// The `FunctionId` of a top-level function by name and parameter
/// type names (overloads share a name).
fn top_level_fn(module: &hir::Module, name: &str, param_tys: &[&str]) -> hir::FunctionId {
    module
        .top_level
        .iter()
        .copied()
        .find(|&id| {
            let f = &module.functions[id];
            f.name == name
                && f.params.len() == param_tys.len()
                && f.params
                    .iter()
                    .zip(param_tys)
                    .all(|(p, want)| hir::type_name(module, p.ty) == *want)
        })
        .unwrap_or_else(|| panic!("no top-level function `{name}` with params {param_tys:?}"))
}

/// The `FunctionId` of a method of `owner` by short name and declared
/// (non-`this`) parameter type names.
fn method_fn(module: &hir::Module, owner: &str, name: &str, param_tys: &[&str]) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find(|(_, f)| {
            f.name == format!("{owner}.{name}")
                && f.params.len() == param_tys.len() + 1
                && f.params[1..]
                    .iter()
                    .zip(param_tys)
                    .all(|(p, want)| hir::type_name(module, p.ty) == *want)
        })
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no method `{owner}.{name}` with params {param_tys:?}"))
}

fn body_of(module: &hir::Module, function: hir::FunctionId) -> &hir::Body {
    match &module.functions[function].kind {
        hir::FunctionKind::User(body) => body,
        _ => panic!("expected a user function"),
    }
}

/// The call inside `main`'s `index`-th statement (`f(...)` as a
/// statement, or `println(inner)` where `inner` is the interesting
/// call when `unnest` is set).
fn call_in_main(
    module: &hir::Module,
    index: usize,
    unnest: bool,
) -> (hir::FunctionId, &[hir::Expr]) {
    let body = body_of(module, module.entry);
    let hir::StatementKind::Expr(expr) = &body.statements[index].kind else {
        panic!("statement {index} is not an expression statement")
    };
    let hir::ExprKind::Call { callee, args } = &expr.kind else {
        panic!("statement {index} is not a call")
    };
    if unnest {
        // `print` / `println` take `Any`: a value-typed result arrives
        // under a `Box` adaptation node.
        let inner = match &args[0].kind {
            hir::ExprKind::Box(operand) => &operand.kind,
            kind => kind,
        };
        let hir::ExprKind::Call { callee, args } = inner else {
            panic!("the argument of statement {index} is not a call")
        };
        (module.callable_function(*callee), args)
    } else {
        (module.callable_function(*callee), args)
    }
}

fn has_instantiation(
    module: &hir::Module,
    function: hir::FunctionId,
    type_args: &[hir::TypeId],
) -> bool {
    module.instantiations.iter().any(|(_, resolved)| {
        module.generic_functions[resolved.generic].function == function
            && resolved.type_args == type_args
    })
}

fn has_method_application(
    module: &hir::Module,
    function: hir::FunctionId,
    owner_arguments: &[hir::TypeId],
) -> bool {
    module.method_applications.iter().any(|(_, application)| {
        if application.function != function {
            return false;
        }
        let arguments: &[hir::TypeId] = match application.owner {
            hir::MethodOwnerApplication::Class(owner) => {
                &module.class_applications[owner].arguments
            }
            hir::MethodOwnerApplication::Struct(owner) => {
                &module.struct_applications[owner].arguments
            }
            hir::MethodOwnerApplication::Enum(owner) => &module.enum_applications[owner].arguments,
            hir::MethodOwnerApplication::Interface(owner) => {
                &module.interface_applications[owner].arguments
            }
            hir::MethodOwnerApplication::Any => &[],
        };
        arguments == owner_arguments
    })
}

// --- declaration rules (DESIGN 1.1) ---

#[test]
fn distinguishable_overloads_are_accepted() {
    let file = file(vec![
        fun_expr(
            "show",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("int"),
        ),
        fun_expr(
            "show",
            vec![],
            vec![("value", ty_named("String"))],
            Some(ty_named("String")),
            str_lit("string"),
        ),
        fun_expr(
            "show",
            vec![],
            vec![("value", ty_named("Int")), ("extra", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("two"),
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("distinguishable overloads must lower");
    top_level_fn(&module, "show", &["Int"]);
    top_level_fn(&module, "show", &["String"]);
    top_level_fn(&module, "show", &["Int", "Int"]);
}

#[test]
fn same_signature_duplicate_is_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        fun_expr(
            "f",
            vec![],
            vec![("y", ty_named("Int"))],
            Some(ty_named("Int")),
            var("y"),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("same-signature overloads must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `f` is already declared with the same signature"
    );
}

#[test]
fn differing_only_in_return_type_is_a_duplicate() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("s"),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("return-type-only difference must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `f` is already declared with the same signature"
    );
}

#[test]
fn same_signature_method_duplicate_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![
                method_expr(
                    "m",
                    vec![("x", ty_named("Int"))],
                    Some(ty_named("Int")),
                    var("x"),
                ),
                method_expr(
                    "m",
                    vec![("y", ty_named("Int"))],
                    Some(ty_named("Int")),
                    var("y"),
                ),
            ],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("same-signature methods must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `m` in class `C` is already declared with the same signature"
    );
}

#[test]
fn override_with_unmatched_signature_is_an_error() {
    // An overload of `m` exists in the base class, but with a
    // different signature — it is not overridden.
    let file = file(vec![
        class_decl(
            Open,
            "B",
            vec![],
            None,
            vec![],
            vec![method_expr(
                "m",
                vec![("x", ty_named("Int"))],
                Some(ty_named("String")),
                str_lit("i"),
            )],
        ),
        class_decl(
            Final,
            "C",
            vec![],
            Some(("B", vec![])),
            vec![],
            vec![override_method_expr(
                "m",
                vec![("x", ty_named("String"))],
                Some(ty_named("String")),
                var("x"),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("unmatched override must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`m` is marked `override` but does not override any method"
    );
}

// --- resolution: applicability (DESIGN 1.2, steps 1-2) ---

#[test]
fn resolves_by_arity_and_type() {
    let show_overloads = vec![
        fun_expr(
            "show",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("int"),
        ),
        fun_expr(
            "show",
            vec![],
            vec![("value", ty_named("String"))],
            Some(ty_named("String")),
            str_lit("string"),
        ),
        fun_expr(
            "show",
            vec![],
            vec![("value", ty_named("Int")), ("extra", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("two"),
        ),
    ];
    let file = file(
        show_overloads
            .into_iter()
            .chain([fun(
                "main",
                vec![
                    stmt(call("println", vec![call("show", vec![int_lit(1)])])),
                    stmt(call("println", vec![call("show", vec![str_lit("a")])])),
                    stmt(call(
                        "println",
                        vec![call("show", vec![int_lit(1), int_lit(2)])],
                    )),
                ],
            )])
            .collect(),
    );
    let module = lower_user(file).expect("overload resolution must succeed");
    let (first, _) = call_in_main(&module, 0, true);
    assert_eq!(first, top_level_fn(&module, "show", &["Int"]));
    let (second, _) = call_in_main(&module, 1, true);
    assert_eq!(second, top_level_fn(&module, "show", &["String"]));
    let (third, _) = call_in_main(&module, 2, true);
    assert_eq!(third, top_level_fn(&module, "show", &["Int", "Int"]));
}

#[test]
fn no_applicable_overload_lists_argument_types() {
    let overloads = vec![
        fun_expr("f", vec![], vec![("x", ty_named("Int"))], None, unit_lit()),
        fun_expr(
            "f",
            vec![],
            vec![("s", ty_named("String"))],
            None,
            unit_lit(),
        ),
    ];
    let file = file(
        overloads
            .into_iter()
            .chain([fun("main", vec![stmt(call("f", vec![bool_lit(true)]))])])
            .collect(),
    );
    let errors = lower_user(file).expect_err("no applicable overload must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no overload of `f` matches argument types (Boolean)"
    );
}

#[test]
fn mixed_arity_no_match_lists_argument_types() {
    let overloads = vec![
        fun_expr("f", vec![], vec![("x", ty_named("Int"))], None, unit_lit()),
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
            None,
            unit_lit(),
        ),
    ];
    let file = file(
        overloads
            .into_iter()
            .chain([fun("main", vec![stmt(call("f", vec![str_lit("s")]))])])
            .collect(),
    );
    let errors = lower_user(file).expect_err("no applicable overload must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no overload of `f` matches argument types (String)"
    );
}

#[test]
fn uniform_arity_mismatch_keeps_the_arity_message() {
    let overloads = vec![
        fun_expr("f", vec![], vec![("x", ty_named("Int"))], None, unit_lit()),
        fun_expr(
            "f",
            vec![],
            vec![("s", ty_named("String"))],
            None,
            unit_lit(),
        ),
    ];
    let file = file(
        overloads
            .into_iter()
            .chain([fun("main", vec![stmt(call("f", vec![]))])])
            .collect(),
    );
    let errors = lower_user(file).expect_err("arity mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`f` takes exactly 1 argument, but 0 were supplied"
    );
}

// --- resolution: most specific candidate (DESIGN 1.2, step 3) ---

/// `open class Shape(name) : Describable`, `class Circle(r) : Shape("c")`,
/// and `tag(Shape)` / `tag(Describable)` overloads (the DESIGN.md 1
/// example): `tag(Circle(1))` must pick the `Shape` overload.
#[test]
fn most_specific_subtype_wins() {
    let file = file(vec![
        interface_decl(
            "Describable",
            vec![bodyless_method(
                false,
                "describe",
                vec![],
                Some(ty_named("String")),
            )],
        ),
        class_decl(
            Open,
            "Shape",
            vec![(false, "name", ty_named("String"))],
            None,
            vec!["Describable"],
            vec![override_method_expr(
                "describe",
                vec![],
                Some(ty_named("String")),
                var("name"),
            )],
        ),
        class_decl(
            Final,
            "Circle",
            vec![(false, "r", ty_named("Int"))],
            Some(("Shape", vec![str_lit("c")])),
            vec![],
            vec![],
        ),
        fun_expr(
            "tag",
            vec![],
            vec![("s", ty_named("Shape"))],
            Some(ty_named("String")),
            str_lit("shape"),
        ),
        fun_expr(
            "tag",
            vec![],
            vec![("d", ty_named("Describable"))],
            Some(ty_named("String")),
            str_lit("desc"),
        ),
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![call("tag", vec![struct_init("Circle", vec![int_lit(1)])])],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("subtype resolution must succeed");
    let (target, _) = call_in_main(&module, 0, true);
    assert_eq!(target, top_level_fn(&module, "tag", &["Shape"]));
}

/// No separate boxing preference rule: `Int <: Any` makes the `Int`
/// overload dominate naturally; an `Any`-typed argument only matches
/// the `Any` overload (and needs no adaptation there).
#[test]
fn boxing_candidate_is_naturally_less_specific() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("int"),
        ),
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Any"))],
            Some(ty_named("String")),
            str_lit("any"),
        ),
        fun(
            "main",
            vec![
                val_ty("a", Some(ty_named("Any")), int_lit(1)),
                stmt(call("println", vec![call("f", vec![int_lit(1)])])),
                stmt(call("println", vec![call("f", vec![var("a")])])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("boxing resolution must succeed");
    let (first, args) = call_in_main(&module, 1, true);
    assert_eq!(first, top_level_fn(&module, "f", &["Int"]));
    // The exact-match overload takes the literal unboxed.
    assert!(matches!(args[0].kind, hir::ExprKind::IntLiteral(1)));
    let (second, _) = call_in_main(&module, 2, true);
    assert_eq!(second, top_level_fn(&module, "f", &["Any"]));
}

/// `f(A, B)` and `f(B, A)` with `B <: A` are mutually non-dominating
/// for a `(B, B)` call — ambiguous.
#[test]
fn mutually_non_dominating_overloads_are_ambiguous() {
    let file = file(vec![
        class_decl(Open, "A", vec![], None, vec![], vec![]),
        class_decl(Final, "B", vec![], Some(("A", vec![])), vec![], vec![]),
        fun_expr(
            "f",
            vec![],
            vec![("a", ty_named("A")), ("b", ty_named("B"))],
            Some(ty_named("String")),
            str_lit("ab"),
        ),
        fun_expr(
            "f",
            vec![],
            vec![("a", ty_named("B")), ("b", ty_named("A"))],
            Some(ty_named("String")),
            str_lit("ba"),
        ),
        fun(
            "main",
            vec![stmt(call(
                "f",
                vec![struct_init("B", vec![]), struct_init("B", vec![])],
            ))],
        ),
    ]);
    let errors = lower_user(file).expect_err("the call must be ambiguous");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "call to `f` is ambiguous");
}

/// A generic and a non-generic candidate dominating each other
/// (the generic one with its inferred type arguments) is a tie,
/// broken in favor of the non-generic one; arguments the non-generic
/// candidate does not match still go to the generic one.
#[test]
fn non_generic_wins_ties_against_generic() {
    let file = file(vec![
        fun_expr(
            "id",
            vec!["T"],
            vec![("x", ty_named("T"))],
            Some(ty_named("T")),
            var("x"),
        ),
        fun_expr(
            "id",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        fun(
            "main",
            vec![
                stmt(call("println", vec![call("id", vec![int_lit(1)])])),
                stmt(call("println", vec![call("id", vec![str_lit("s")])])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("generic overloads must resolve");
    let generic = top_level_fn(&module, "id", &["T0"]);
    let concrete = top_level_fn(&module, "id", &["Int"]);

    // `id(1)`: the non-generic candidate wins the tie; no type
    // arguments, no instantiation request.
    let (first, _) = call_in_main(&module, 0, true);
    assert_eq!(first, concrete);
    let body = body_of(&module, module.entry);
    let hir::StatementKind::Expr(outer) = &body.statements[0].kind else {
        panic!("expected a call statement")
    };
    let hir::ExprKind::Call { args, .. } = &outer.kind else {
        panic!("expected a call")
    };
    let hir::ExprKind::Box(nested) = &args[0].kind else {
        panic!("the Int result must be boxed into `Any`")
    };
    let hir::ExprKind::Call { callee, .. } = &nested.kind else {
        panic!("expected a nested call")
    };
    assert!(matches!(callee, hir::Callable::Function(id) if *id == concrete));

    // `id("s")`: only the generic candidate is applicable, with
    // `T = String` inferred and recorded.
    let (second, _) = call_in_main(&module, 1, true);
    assert_eq!(second, generic);
    assert!(has_instantiation(&module, generic, &[module.string]));
    assert!(!has_instantiation(&module, generic, &[module.int]));
}

// --- method overloads and layering (DESIGN 1.2, step 1) ---

/// Method overloads resolve by the same algorithm, for explicit
/// receivers (`c.m("a")`) and bare calls inside a method body
/// (`m(1)` meaning `this.m(1)`).
#[test]
fn method_overloads_resolve() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![
                method_expr(
                    "m",
                    vec![("x", ty_named("Int"))],
                    Some(ty_named("String")),
                    str_lit("int"),
                ),
                method_expr(
                    "m",
                    vec![("x", ty_named("String"))],
                    Some(ty_named("String")),
                    str_lit("string"),
                ),
                method_expr(
                    "probe",
                    vec![],
                    Some(ty_named("String")),
                    call("m", vec![int_lit(1)]),
                ),
            ],
        ),
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![method_call(
                    struct_init("C", vec![]),
                    "m",
                    vec![str_lit("a")],
                )],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("method overloads must resolve");

    // `c.m("a")` picks the `String` overload.
    let body = body_of(&module, module.entry);
    let hir::StatementKind::Expr(outer) = &body.statements[0].kind else {
        panic!("expected a call statement")
    };
    let hir::ExprKind::Call { args, .. } = &outer.kind else {
        panic!("expected a call")
    };
    let hir::ExprKind::MethodCall { callee, .. } = &args[0].kind else {
        panic!("expected a method call")
    };
    assert_eq!(
        module.callable_function(*callee),
        method_fn(&module, "C", "m", &["String"])
    );

    // The bare `m(1)` inside `probe` is `this.m(1)` and picks the
    // `Int` overload.
    let probe = method_fn(&module, "C", "probe", &[]);
    let body = body_of(&module, probe);
    let hir::StatementKind::Return { value: Some(value) } = &body.statements[0].kind else {
        panic!("expected a return")
    };
    let hir::ExprKind::MethodCall {
        callee, receiver, ..
    } = &value.kind
    else {
        panic!("expected a method call")
    };
    assert_eq!(
        module.callable_function(*callee),
        method_fn(&module, "C", "m", &["Int"])
    );
    assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
}

/// The member layer wins whole over the top-level layer: inside `C`'s
/// methods a bare `value()` is the member even though a same-name
/// top-level function exists.
#[test]
fn member_layer_shadows_top_level() {
    let file = file(vec![
        fun_expr("value", vec![], vec![], Some(ty_named("Int")), int_lit(1)),
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![
                method_expr("value", vec![], Some(ty_named("Int")), int_lit(2)),
                method_expr(
                    "probe",
                    vec![],
                    Some(ty_named("Int")),
                    call("value", vec![]),
                ),
            ],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("member shadowing must resolve");
    let probe = method_fn(&module, "C", "probe", &[]);
    let body = body_of(&module, probe);
    let hir::StatementKind::Return { value: Some(value) } = &body.statements[0].kind else {
        panic!("expected a return")
    };
    let hir::ExprKind::MethodCall { callee, .. } = &value.kind else {
        panic!(
            "the bare call must resolve to the member, found {:?}",
            value.kind
        )
    };
    assert_eq!(
        module.callable_function(*callee),
        method_fn(&module, "C", "value", &[])
    );
}

/// The user layer wins whole over the core implicit-import layer for
/// calls from the user file: a user `print()` shadows all three core
/// `print` overloads there.
#[test]
fn user_layer_shadows_core_overloads() {
    let file = file(vec![
        fun_expr(
            "print",
            vec![],
            vec![],
            None,
            call("write", vec![str_lit("user")]),
        ),
        fun("main", vec![stmt(call("print", vec![]))]),
    ]);
    let module = lower_user(file).expect("user shadowing must resolve");
    let (target, _) = call_in_main(&module, 0, false);
    // The user's zero-parameter `print` (declared in file 1), not a
    // core overload.
    assert_eq!(target, top_level_fn(&module, "print", &[]));
}

/// The discarded layer really is gone: with only the user's `print()`
/// in scope, `print("x")` has no matching overload even though core
/// declares `print(String)`.
#[test]
fn shadowed_core_overloads_do_not_participate() {
    let file = file(vec![
        fun_expr(
            "print",
            vec![],
            vec![],
            None,
            call("write", vec![str_lit("user")]),
        ),
        fun("main", vec![stmt(call("print", vec![str_lit("x")]))]),
    ]);
    let errors = lower_user(file).expect_err("the core overloads must be discarded");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `print` takes exactly 0 arguments, but 1 were supplied"
    );
}

/// The layering is relative to the call site's file
/// (tests/fixtures/m7-overload/overload-core.scoop): the user's
/// `write(Int)` shadows core's `write` overloads for calls from the
/// user file, but the core library's own bodies still resolve their
/// internal calls against the core layer.
#[test]
fn layering_is_relative_to_the_call_site_file() {
    let file = file(vec![
        fun_expr(
            "write",
            vec![],
            vec![("v", ty_named("Int"))],
            Some(ty_named("String")),
            str_lit("user-write"),
        ),
        fun(
            "main",
            vec![
                stmt(call("println", vec![call("write", vec![int_lit(1)])])),
                stmt(call("println", vec![str_lit("c")])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("call-site-relative layering must resolve");

    // The user call `write(1)` picks the user's `write(Int)` (same
    // side; core's `write(String)` is discarded).
    let (target, _) = call_in_main(&module, 0, true);
    assert_eq!(target, top_level_fn(&module, "write", &["Int"]));

    // Core's `print(Any)` body still calls the core managed `write`
    // extern — the user
    // overload does not leak into core's own layer.
    let core_write = module
        .top_level
        .iter()
        .copied()
        .find(|&id| {
            let f = &module.functions[id];
            f.name == "write" && matches!(f.kind, hir::FunctionKind::Extern(_))
        })
        .expect("core declares the write extern");
    let core_print = top_level_fn(&module, "print", &["Any"]);
    let body = body_of(&module, core_print);
    let hir::StatementKind::Expr(value) = &body.statements[0].kind else {
        panic!("expected the call statement")
    };
    let hir::ExprKind::Call { callee, .. } = &value.kind else {
        panic!("expected a call")
    };
    assert_eq!(module.callable_function(*callee), core_write);

    // Core's `println` body is likewise unaffected: both of its
    // `write` calls target the core extern.
    let core_println = top_level_fn(&module, "println", &["Any"]);
    let body = body_of(&module, core_println);
    for statement in &body.statements {
        let hir::StatementKind::Expr(value) = &statement.kind else {
            panic!("expected a call statement")
        };
        let hir::ExprKind::Call { callee, .. } = &value.kind else {
            panic!("expected a call")
        };
        assert_eq!(module.callable_function(*callee), core_write);
    }
}

// --- core `print` / `println` (DESIGN section 2, final form) ---

/// The core managed `write` extern.
fn core_write(module: &hir::Module) -> hir::FunctionId {
    module
        .top_level
        .iter()
        .copied()
        .find(|&id| {
            let f = &module.functions[id];
            f.name == "write" && matches!(f.kind, hir::FunctionKind::Extern(_))
        })
        .expect("core declares the write extern")
}

/// The synthesized `Any` member by (qualified) name.
fn any_member(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find(|(_, f)| f.name == format!("Any.{name}"))
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("`Any.{name}` is synthesized"))
}

/// `print` / `println` are single `Any`-parameter core functions:
/// every argument type resolves to them (value types arrive boxed,
/// references retyped), and their bodies call `write` with
/// `message.toString()` — a method call on the `Any` receiver
/// resolving to the synthesized `Any.toString` (vtable slot 2).
#[test]
fn print_and_println_take_any_and_dispatch_to_string() {
    let file = file(vec![fun(
        "main",
        vec![
            stmt(call("println", vec![int_lit(42)])),
            stmt(call("println", vec![str_lit("x")])),
            stmt(call("println", vec![bool_lit(true)])),
            stmt(call("print", vec![int_lit(1)])),
        ],
    )]);
    let module = lower_user(file).expect("the core `Any` functions must resolve");
    let println = top_level_fn(&module, "println", &["Any"]);
    let print = top_level_fn(&module, "print", &["Any"]);

    // All four calls resolve to the single core `println` / `print`.
    for (index, want) in [println, println, println, print].into_iter().enumerate() {
        let (target, _) = call_in_main(&module, index, false);
        assert_eq!(target, want);
    }

    // The `Int` argument is boxed into `Any`; the `String` argument is
    // a zero-cost retype (no `Box` node, its type becomes `Any`).
    let body = body_of(&module, module.entry);
    let hir::StatementKind::Expr(first) = &body.statements[0].kind else {
        panic!("expected a call statement")
    };
    let hir::ExprKind::Call { args, .. } = &first.kind else {
        panic!("expected a call")
    };
    assert!(matches!(args[0].kind, hir::ExprKind::Box(_)));
    assert!(matches!(module.types[args[0].ty], Type::Any));
    let hir::StatementKind::Expr(second) = &body.statements[1].kind else {
        panic!("expected a call statement")
    };
    let hir::ExprKind::Call { args, .. } = &second.kind else {
        panic!("expected a call")
    };
    assert!(matches!(args[0].kind, hir::ExprKind::StringLiteral(_)));
    assert!(matches!(module.types[args[0].ty], Type::Any));

    // Core's `print` body is `write(message.toString())`: the write
    // call targets the managed `write` extern, its argument a method
    // call to the synthesized `Any.toString` on the `Any` parameter.
    let write = core_write(&module);
    let to_string = any_member(&module, "toString");
    let body = body_of(&module, print);
    let hir::StatementKind::Expr(value) = &body.statements[0].kind else {
        panic!("expected a call statement")
    };
    let hir::ExprKind::Call { callee, args } = &value.kind else {
        panic!("expected a call")
    };
    assert_eq!(module.callable_function(*callee), write);
    let hir::ExprKind::MethodCall {
        callee, receiver, ..
    } = &args[0].kind
    else {
        panic!("expected a `toString()` method call")
    };
    assert_eq!(module.callable_function(*callee), to_string);
    assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
}

/// The three `Any` members resolve on an `Any` receiver
/// (`synthesize_any_members`; mir-lower dispatches them through the
/// fixed vtable slots 0..2).
#[test]
fn any_receiver_resolves_the_any_members() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_named("Any")), int_lit(1)),
            val_ty("b", Some(ty_named("Any")), str_lit("x")),
            stmt(call(
                "println",
                vec![method_call(var("a"), "toString", vec![])],
            )),
            stmt(call(
                "println",
                vec![method_call(var("a"), "hashCode", vec![])],
            )),
            stmt(call(
                "println",
                vec![method_call(var("a"), "equals", vec![var("b")])],
            )),
        ],
    )]);
    let module = lower_user(file).expect("`Any` member calls must resolve");
    let body = body_of(&module, module.entry);
    for (index, name) in ["toString", "hashCode", "equals"].into_iter().enumerate() {
        let hir::StatementKind::Expr(outer) = &body.statements[index + 2].kind else {
            panic!("expected a call statement")
        };
        let hir::ExprKind::Call { args, .. } = &outer.kind else {
            panic!("expected a call")
        };
        let inner = match &args[0].kind {
            hir::ExprKind::Box(operand) => &operand.kind,
            kind => kind,
        };
        let hir::ExprKind::MethodCall { callee, .. } = inner else {
            panic!("expected a method call")
        };
        assert_eq!(module.callable_function(*callee), any_member(&module, name));
    }
}

/// The fallback is deliberately narrow: value-type receivers (which
/// would need boxing before the virtual call) do not resolve the
/// `Any` members yet.
#[test]
fn any_members_do_not_resolve_on_value_receivers() {
    let file = file(vec![fun(
        "main",
        vec![stmt(call(
            "println",
            vec![method_call(int_lit(1), "toString", vec![])],
        ))],
    )]);
    let errors = lower_user(file).expect_err("`Int` has no `toString` in M7");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "type `Int` has no method `toString`");
}

// --- generic enum method overloads ---

/// Overloads on a generic enum instantiate with the receiver's type
/// arguments before applicability and dominance: `pick(T)` and
/// `pick(Int)` on a `Box<String>` select `pick(T)`; on a `Box<Int>`
/// the two tie after instantiation and the non-generic one wins.
#[test]
fn enum_method_overloads_instantiate_with_the_receiver() {
    let file = file(vec![
        enum_decl_methods(
            "Box",
            vec!["T"],
            vec![variant_positional("V", vec![ty_named("T")])],
            vec![
                method_expr(
                    "pick",
                    vec![("x", ty_named("T"))],
                    Some(ty_named("T")),
                    var("x"),
                ),
                method_expr(
                    "pick",
                    vec![("x", ty_named("Int"))],
                    Some(ty_named("Int")),
                    var("x"),
                ),
            ],
        ),
        fun(
            "main",
            vec![
                val("b", struct_init("Box.V", vec![str_lit("s")])),
                stmt(call(
                    "println",
                    vec![method_call(var("b"), "pick", vec![str_lit("a")])],
                )),
                val("i", struct_init("Box.V", vec![int_lit(1)])),
                stmt(call(
                    "println",
                    vec![method_call(var("i"), "pick", vec![int_lit(2)])],
                )),
            ],
        ),
    ]);
    let module = lower_user(file).expect("enum method overloads must resolve");
    let pick_t = method_fn(&module, "Box", "pick", &["T0"]);
    let pick_int = method_fn(&module, "Box", "pick", &["Int"]);

    let method_target = |index: usize| {
        let body = body_of(&module, module.entry);
        let hir::StatementKind::Expr(outer) = &body.statements[index].kind else {
            panic!("expected a call statement")
        };
        let hir::ExprKind::Call { args, .. } = &outer.kind else {
            panic!("expected a call")
        };
        let inner = match &args[0].kind {
            hir::ExprKind::Box(operand) => &operand.kind,
            kind => kind,
        };
        let hir::ExprKind::MethodCall { callee, .. } = inner else {
            panic!("expected a method call")
        };
        module.callable_function(*callee)
    };
    assert_eq!(method_target(1), pick_t);
    assert_eq!(method_target(3), pick_int);

    // The chosen enum methods request instantiations with the
    // receiver's type arguments.
    assert!(has_method_application(&module, pick_t, &[module.string]));
    assert!(has_method_application(&module, pick_int, &[module.int]));
}

// --- entry point ---

/// Overloads of `main` are ordinary functions; the zero-parameter one
/// is the entry point.
#[test]
fn overloaded_main_entry_is_the_zero_parameter_one() {
    let file = file(vec![
        fun_sig("main", vec![], vec![("x", ty_named("Int"))], None, vec![]),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("overloaded main must lower");
    let entry = &module.functions[module.entry];
    assert_eq!(entry.name, "main");
    assert!(entry.params.is_empty());
}
