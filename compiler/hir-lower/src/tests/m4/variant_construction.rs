use super::*;

// --- negative: variant construction ---

#[test]
fn unknown_enum_in_qualified_path_is_an_error() {
    let file = file(vec![fun("main", vec![val("c", field(var("Foo"), "Red"))])]);
    let errors = lower_user(file).expect_err("unknown enum must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "unknown variable `Foo`; bare enum variants without an exact enum expected type must be qualified as `E.V` or given a type annotation"
    );
}

#[test]
fn unknown_variant_in_qualified_path_is_an_error() {
    let file = file(vec![
        color_decl(),
        fun("main", vec![val("c", field(var("Color"), "Purple"))]),
    ]);
    let errors = lower_user(file).expect_err("unknown variant must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "enum `Color` has no variant `Purple`");
}

#[test]
fn unknown_variant_in_dotted_call_is_an_error() {
    let file = file(vec![
        color_decl(),
        fun("main", vec![val("c", call("Color.Purple", vec![]))]),
    ]);
    let errors = lower_user(file).expect_err("unknown variant must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "enum `Color` has no variant `Purple`");
}

/// A non-prelude variant without an exact enum expected type must be
/// qualified or annotated.
#[test]
fn bare_non_option_variant_is_an_error() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![val("c", var("Red")), val("d", call("Green", vec![]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("bare variants must fail");
    assert_eq!(errors.len(), 2);
    assert_eq!(
        errors[0].message,
        "unknown variable `Red`; bare enum variants without an exact enum expected type must be qualified as `E.V` or given a type annotation"
    );
    assert_eq!(
        errors[1].message,
        "unknown function `Green`; bare enum variants without an exact enum expected type must be qualified as `E.V` or given a type annotation"
    );
}

#[test]
fn variant_arity_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![val("s", call("Shape.Circle", vec![int_lit(1), int_lit(2)]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("too many arguments must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for constructor `Shape.Circle` in nominal constructor candidate layer:\n  - variant Shape.Circle(_1: Int) — expects 1 argument(s), but 2 were supplied"
    );
}

#[test]
fn variant_missing_argument_without_default_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![val(
                "s",
                source_call("Shape.Named", vec![named_argument("w", int_lit(1))]),
            )],
        ),
    ]);
    let errors = lower_user(file).expect_err("missing argument must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for constructor `Shape.Named` in nominal constructor candidate layer:\n  - variant Shape.Named(w: Int, h: Int) — required parameter `h` has no argument"
    );
}

#[test]
fn variant_argument_type_mismatch_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![val("s", call("Shape.Circle", vec![str_lit("x")]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("argument mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for constructor `Shape.Circle` in nominal constructor candidate layer:\n  - variant Shape.Circle(_1: Int) — argument for `_1` has type String, which is not a subtype of Int"
    );
}

/// A qualified unit variant of a generic enum needs an expected type
/// to take its type arguments from.
#[test]
fn qualified_none_needs_an_expected_type() {
    let file = file(vec![fun(
        "main",
        vec![val("o", field(var("Option"), "None"))],
    )]);
    let errors = lower_user(file).expect_err("unhinted `Option.None` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for constructor `Option.None` in nominal constructor candidate layer:\n  - variant Option<T>.None() — cannot infer a unique type argument for `T`"
    );
}

#[test]
fn qualified_none_with_expected_type_lowers() {
    let file = file(vec![fun(
        "main",
        vec![val_ty(
            "o",
            Some(ty_nullable(ty_named("Int"))),
            field(var("Option"), "None"),
        )],
    )]);
    let module = lower_user(file).expect("hinted `Option.None` must lower");
    let dump = hir::dump(&module);
    assert!(
        dump.contains("VariantConstruct Option.None<Int> : Option<Int>"),
        "{dump}"
    );
}

#[test]
fn non_unit_variant_without_call_is_an_error() {
    let file = file(vec![fun("main", vec![val("o", var("Some"))])]);
    let errors = lower_user(file).expect_err("bare `Some` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "variant `Some` of `Option` takes arguments; use `Some(...)` to construct it"
    );
}

#[test]
fn generic_enum_annotation_without_type_arguments_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("o", Some(ty_named("Option")), none())],
    )]);
    let errors = lower_user(file).expect_err("bare generic enum must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "generic enum `Option` requires 1 type argument(s)"
    );
}

#[test]
fn variant_construction_statement_is_not_a_call() {
    let file = file(vec![
        color_decl(),
        fun("main", vec![stmt(field(var("Color"), "Red"))]),
    ]);
    let errors = lower_user(file).expect_err("construction statement must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "statement must be a function call");
}
