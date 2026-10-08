use super::*;

/// `main` calls `println("hello, world")` then `helper()`, which
/// calls `print("!")` — the M1 hello world shape (milestone1
/// DESIGN.md 1).
fn hello_world() -> SourceFile {
    file(vec![
        fun(
            "main",
            vec![
                stmt(call("println", vec![str_lit("hello, world")])),
                stmt(call("helper", vec![])),
            ],
        ),
        fun("helper", vec![stmt(call("print", vec![str_lit("!")]))]),
    ])
}

#[test]
fn lowers_hello_world() {
    let module = lower_user(hello_world()).expect("hello world must lower");

    // Well-known types are allocated first, in a fixed order.
    assert_eq!(module.types[module.unit], Type::Unit);
    assert_eq!(
        module.types[int_type(&module)],
        Type::Integer(hir::IntegerKind::SIGNED_32)
    );
    assert_eq!(module.types[module.boolean], Type::Boolean);
    assert_eq!(module.types[module.string], Type::String);
    // `Option` comes from the core library.
    assert_eq!(
        module.enums[defined_export_core(&module).option.enumeration()].name,
        "Option"
    );

    // Entry point is `main`.
    assert_eq!(module.functions[module.entry()].name, "main");

    // Golden dump locks the output structure.
    let expected = include_str!("snapshots/lowers_hello_world.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn duplicate_function_is_an_error() {
    let file = file(vec![fun("main", vec![]), fun("main", vec![])]);
    let errors = lower_user(file).expect_err("duplicate `main` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `main` is already declared with the same signature"
    );
    // The duplicate is in the user file (index 1; core is index 0).
    assert_eq!(errors[0].file, 1);
}

#[test]
fn current_package_may_shadow_a_core_prelude_function() {
    // The current package and the core prelude are distinct lookup layers, so
    // an otherwise identical declaration in the current package is legal.
    let mut duplicate = fun_expr(
        "print",
        vec!["T"],
        vec![("value", ty_named("T"))],
        None,
        unit_lit(),
    );
    let Decl::Function(function) = &mut duplicate else {
        unreachable!()
    };
    function.type_params[0].inline_bound = Some(ast::TypeBound::Upper(ty_named("ToString")));
    let file = file(vec![fun("main", vec![]), duplicate]);
    lower_user(file).expect("the current package may shadow the core prelude");
}

#[test]
fn unknown_function_is_an_error_with_callee_span() {
    let callee_span = Span::new(10, 15);
    let file = file(vec![fun(
        "main",
        vec![stmt(call_at("hello", vec![], callee_span))],
    )]);
    let errors = lower_user(file).expect_err("unknown callee must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "unknown function `hello`; bare enum variants without an exact enum expected type must be qualified as `E.V` or given a type annotation"
    );
    assert_eq!(errors[0].span, Some(callee_span));
}

#[test]
fn print_requires_exactly_one_argument() {
    for args in [vec![], vec![str_lit("a"), str_lit("b")]] {
        let supplied = args.len();
        let file = file(vec![fun("main", vec![stmt(call("print", args))])]);
        let errors = lower_user(file).expect_err("wrong arity must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            format!(
                "no applicable candidate for `print` in implicit-import candidate layer:\n  - fun print<T : ToString>(value: T): Unit — expects 1 argument(s), but {supplied} were supplied"
            )
        );
    }
}

#[test]
fn user_function_arity_is_an_error() {
    let file = file(vec![
        fun("main", vec![stmt(call("helper", vec![str_lit("x")]))]),
        fun("helper", vec![]),
    ]);
    let errors = lower_user(file).expect_err("argument to `helper` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for `helper` in current-unit top-level candidate layer:\n  - fun helper(): Unit — expects 0 argument(s), but 1 were supplied"
    );
}

#[test]
fn missing_main_is_an_error_with_current_source_anchor() {
    let file = file(vec![fun("helper", vec![])]);
    let errors = lower_user(file.clone()).expect_err("missing `main` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "missing executable entry: declare exactly one ordinary `fun main(): Unit`, `fun main(): Int`, `fun main(args: Array<String>): Unit`, or `fun main(args: Array<String>): Int`"
    );
    assert_eq!(errors[0].span, Some(Span::new(0, 0)));
}

#[test]
fn bare_literal_statement_is_an_error() {
    let file = file(vec![fun("main", vec![stmt(str_lit("dangling"))])]);
    let errors = lower_user(file).expect_err("literal statement must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "statement must be a function call");
}

#[test]
fn collects_multiple_diagnostics() {
    let file = file(vec![fun(
        "main",
        vec![
            stmt(call("missing_one", vec![])),
            stmt(call("missing_two", vec![])),
        ],
    )]);
    let errors = lower_user(file).expect_err("unknown callees must fail");
    assert_eq!(errors.len(), 2);
    assert_eq!(
        errors[0].message,
        "unknown function `missing_one`; bare enum variants without an exact enum expected type must be qualified as `E.V` or given a type annotation"
    );
    assert_eq!(
        errors[1].message,
        "unknown function `missing_two`; bare enum variants without an exact enum expected type must be qualified as `E.V` or given a type annotation"
    );
}
