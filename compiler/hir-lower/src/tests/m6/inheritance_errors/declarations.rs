use super::*;

// --- negative: declarations and inheritance ---

#[test]
fn inheriting_a_final_class_is_an_error() {
    let file = file(vec![
        class_decl(Final, "A", vec![], None, vec![], vec![]),
        class_decl(Final, "B", vec![], Some(("A", vec![])), vec![], vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("inheriting a final class must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "class `A` is final and cannot be inherited"
    );
}

#[test]
fn interface_supertype_cannot_have_constructor_arguments() {
    let file = file(vec![
        describable(),
        class_decl(
            Final,
            "B",
            vec![],
            Some(("Describable", vec![])),
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("interface constructor arguments must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "interfaces cannot have constructor arguments"
    );
}

#[test]
fn bare_class_supertype_is_classified_as_the_base() {
    let file = file(vec![
        class_decl(Final, "A", vec![], None, vec![], vec![]),
        class_decl(Open, "B", vec![], None, vec!["A"], vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("inheriting a final bare class must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "class `A` is final and cannot be inherited"
    );
}

#[test]
fn duplicate_constructor_property_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int")), (true, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate properties must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate property `x` in class `C`");
}

#[test]
fn duplicate_method_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![
                method("m", vec![], None, vec![]),
                method("m", vec![], None, vec![]),
            ],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate methods must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `m` in class `C` is already declared with the same signature"
    );
}

#[test]
fn cyclic_inheritance_is_an_error() {
    let file = file(vec![
        class_decl(Open, "A", vec![], Some(("B", vec![])), vec![], vec![]),
        class_decl(Open, "B", vec![], Some(("A", vec![])), vec![], vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("cyclic inheritance must fail");
    assert!(
        errors
            .iter()
            .any(|e| e.message == "class `A` directly or indirectly inherits from itself"),
        "unexpected diagnostics: {errors:?}"
    );
}

#[test]
fn property_shadowing_is_an_error() {
    let file = file(vec![
        class_decl(
            Open,
            "A",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        class_decl(
            Final,
            "B",
            vec![(false, "x", ty_named("Int"))],
            Some(("A", vec![int_lit(1)])),
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("shadowing properties must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "property `x` cannot override final property declared by class `A`"
    );
}

#[test]
fn base_constructor_arity_is_checked() {
    let file = file(vec![
        class_decl(
            Open,
            "A",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        class_decl(
            Final,
            "B",
            vec![],
            Some(("A", vec![int_lit(1), int_lit(2)])),
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("wrong arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for constructor `A` in nominal constructor candidate layer:\n  - class A(x: Int) — expects 1 argument(s), but 2 were supplied"
    );
}

#[test]
fn base_constructor_argument_types_are_checked() {
    let file = file(vec![
        class_decl(
            Open,
            "A",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        class_decl(
            Final,
            "B",
            vec![],
            Some(("A", vec![str_lit("s")])),
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("wrong argument type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for constructor `A` in nominal constructor candidate layer:\n  - class A(x: Int) — argument for `x` has type String, which is not a subtype of Int"
    );
}

// --- negative: override and implementation ---
