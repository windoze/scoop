use super::*;

// --- negative: literal inference ---

#[test]
fn empty_literal_requires_an_expected_type() {
    // No annotation at all.
    let file = file(vec![fun("main", vec![val("e", array_lit(vec![]))])]);
    let errors = lower_user(file).expect_err("context-free `[]` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cannot infer the element type of an empty array literal"
    );

    // An expected type that is not an array does not help either.
    let file2 = super::file(vec![fun(
        "main",
        vec![val_ty("e", Some(ty_named("Int")), array_lit(vec![]))],
    )]);
    let errors = lower_user(file2).expect_err("`[]` under `Int` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cannot infer the element type of an empty array literal"
    );
}

#[test]
fn mixed_element_types_are_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val("x", array_lit(vec![int_lit(1), str_lit("a")]))],
    )]);
    let errors = lower_user(file).expect_err("mixed elements must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "array literal elements must have the same type, found Int and String"
    );
}

#[test]
fn reference_elements_infer_their_representable_lob() {
    use ast::ClassModifier::{Final, Open};

    let file = file(vec![
        class_decl(Open, "Base", vec![], None, vec![], vec![]),
        class_decl(Final, "A", vec![], Some(("Base", vec![])), vec![], vec![]),
        class_decl(Final, "B", vec![], Some(("Base", vec![])), vec![], vec![]),
        interface_decl("I", vec![]),
        interface_decl("J", vec![]),
        class_decl(Final, "C", vec![], None, vec!["I"], vec![]),
        class_decl(Final, "D", vec![], None, vec!["I"], vec![]),
        class_decl(Final, "E", vec![], None, vec!["I", "J"], vec![]),
        class_decl(Final, "F", vec![], None, vec!["I", "J"], vec![]),
        fun(
            "main",
            vec![
                val(
                    "classes",
                    array_lit(vec![call("A", vec![]), call("B", vec![])]),
                ),
                val(
                    "interfaces",
                    array_lit(vec![call("C", vec![]), call("D", vec![])]),
                ),
                val(
                    "unrelated",
                    array_lit(vec![call("A", vec![]), str_lit("text")]),
                ),
                val(
                    "ambiguous",
                    array_lit(vec![call("E", vec![]), call("F", vec![])]),
                ),
            ],
        ),
    ]);
    let module = lower_user(file).expect("reference LOB inference must lower");
    let body = match &module.functions[module.entry()].kind {
        hir::FunctionKind::User(body) => body,
        hir::FunctionKind::Intrinsic(_)
        | hir::FunctionKind::Extern(_)
        | hir::FunctionKind::DerivedEquality => {
            panic!("main is a user function")
        }
    };

    for (statement, expected) in
        body.statements
            .iter()
            .zip(["Array<Base>", "Array<I>", "Array<Any>", "Array<Any>"])
    {
        let hir::StatementKind::ValDecl { init, .. } = &statement.kind else {
            panic!("expected a val declaration")
        };
        assert_eq!(hir::type_name(&module, init.ty), expected);
        let hir::ExprKind::ArrayLiteral(elements) = &init.kind else {
            panic!("expected an array literal")
        };
        let element_ty = hir::type_name(&module, elements[0].ty);
        assert!(
            elements
                .iter()
                .all(|element| hir::type_name(&module, element.ty) == element_ty)
        );
        assert!(
            elements
                .iter()
                .all(|element| !matches!(element.kind, hir::ExprKind::Box(_)))
        );
    }
}

#[test]
fn expected_array_type_does_not_auto_box_value_elements() {
    let file = file(vec![
        interface_decl("I", vec![]),
        struct_decl_full("S", vec![("v", ty_named("Int"))], vec!["I"], vec![]),
        fun(
            "main",
            vec![val_ty(
                "values",
                Some(ty_generic("Array", vec![ty_named("I")])),
                array_lit(vec![struct_init("S", vec![int_lit(1)])]),
            )],
        ),
    ]);
    let errors = lower_user(file).expect_err("array literals must not auto-box values");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "array literal element must be of type I, found S"
    );
}

#[test]
fn element_must_match_the_expected_element_type() {
    let file = file(vec![fun(
        "main",
        vec![val_ty(
            "m",
            Some(ty_mutable_int_array()),
            array_lit(vec![int_lit(1), str_lit("a")]),
        )],
    )]);
    let errors = lower_user(file).expect_err("element mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "array literal element must be of type Int, found String"
    );
}
