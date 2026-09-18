use super::*;

#[test]
fn open_and_default_open_override_form_one_visible_dispatch_slot() {
    let open_f = with_method_modifier(
        method_expr("f", vec![], Some(ty_named("Int")), int_lit(1)),
        ast::MethodModifier::Open,
    );
    let file = file(vec![
        class_decl(Open, "A", vec![], None, vec![], vec![open_f]),
        class_decl(
            Open,
            "B",
            vec![],
            Some(("A", vec![])),
            vec![],
            vec![override_method_expr(
                "f",
                vec![],
                Some(ty_named("Int")),
                int_lit(2),
            )],
        ),
        class_decl(
            Final,
            "C",
            vec![],
            Some(("B", vec![])),
            vec![],
            vec![override_method_expr(
                "f",
                vec![],
                Some(ty_named("Int")),
                int_lit(3),
            )],
        ),
        fun(
            "main",
            vec![
                val_ty("a", Some(ty_named("A")), call("C", vec![])),
                stmt(call("println", vec![method_call(var("a"), "f", vec![])])),
            ],
        ),
    ]);
    let output = lower_user_output(file).expect("the override chain must lower without ambiguity");
    let module = &output.export;
    assert_eq!(
        module.functions[find_fn(module, "A.f")]
            .method
            .expect("method")
            .modifier,
        hir::MethodModifier::Open
    );
    assert_eq!(
        module.functions[find_fn(module, "B.f")]
            .method
            .expect("method")
            .modifier,
        hir::MethodModifier::Open
    );
    // The owner class is final, so its otherwise-open override is
    // normalized to an effectively final method in HIR.
    assert_eq!(
        module.functions[find_fn(module, "C.f")]
            .method
            .expect("method")
            .modifier,
        hir::MethodModifier::Final
    );
    let hir::MethodDispatch::Virtual(family) = module.functions[find_fn(module, "A.f")]
        .method
        .expect("method")
        .dispatch
    else {
        panic!("the first open declaration owns a virtual family")
    };
    assert_eq!(
        module.functions[find_fn(module, "B.f")]
            .method
            .expect("method")
            .dispatch,
        hir::MethodDispatch::Virtual(family)
    );
    assert_eq!(
        module.functions[find_fn(module, "C.f")]
            .method
            .expect("method")
            .dispatch,
        hir::MethodDispatch::FinalOverride(family)
    );

    let dispatches = ["A.f", "B.f", "C.f"].map(|name| {
        output
            .local
            .functions
            .iter()
            .find_map(|(_, function)| (function.name == name).then_some(function.receiver.method()))
            .flatten()
            .expect("the concrete override chain keeps method metadata")
            .dispatch
    });
    let hir::concrete::MethodDispatch::Virtual(local_family) = dispatches[0] else {
        panic!("the concrete base method owns a virtual family")
    };
    assert_eq!(
        dispatches[1],
        hir::concrete::MethodDispatch::Virtual(local_family)
    );
    assert_eq!(
        dispatches[2],
        hir::concrete::MethodDispatch::FinalOverride(local_family)
    );
}

#[test]
fn overriding_a_final_method_is_an_error() {
    let file = file(vec![
        class_decl(
            Open,
            "A",
            vec![],
            None,
            vec![],
            vec![method_expr("f", vec![], Some(ty_named("Int")), int_lit(1))],
        ),
        class_decl(
            Final,
            "B",
            vec![],
            Some(("A", vec![])),
            vec![],
            vec![override_method_expr(
                "f",
                vec![],
                Some(ty_named("Int")),
                int_lit(2),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a final method must not be overridden");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`f` cannot override final method `A.f`");
}

#[test]
fn final_override_closes_the_override_chain() {
    let open_f = with_method_modifier(
        method_expr("f", vec![], Some(ty_named("Int")), int_lit(1)),
        ast::MethodModifier::Open,
    );
    let final_override = with_method_modifier(
        override_method_expr("f", vec![], Some(ty_named("Int")), int_lit(2)),
        ast::MethodModifier::Final,
    );
    let file = file(vec![
        class_decl(Open, "A", vec![], None, vec![], vec![open_f]),
        class_decl(
            Open,
            "B",
            vec![],
            Some(("A", vec![])),
            vec![],
            vec![final_override],
        ),
        class_decl(
            Final,
            "C",
            vec![],
            Some(("B", vec![])),
            vec![],
            vec![override_method_expr(
                "f",
                vec![],
                Some(ty_named("Int")),
                int_lit(3),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("final override must close the slot");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`f` cannot override final method `B.f`");
}

#[test]
fn fresh_open_method_requires_an_inheritable_class() {
    let open_f = with_method_modifier(method("f", vec![], None, vec![]), ast::MethodModifier::Open);
    let file = file(vec![
        class_decl(Final, "C", vec![], None, vec![], vec![open_f]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a final class cannot introduce an open method");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "open function `f` is only allowed in open or abstract classes"
    );
}

#[test]
fn value_types_cannot_declare_open_methods() {
    let open_f = with_method_modifier(method("f", vec![], None, vec![]), ast::MethodModifier::Open);
    let file = file(vec![
        struct_decl_methods("S", vec![("v", ty_named("Int"))], vec![open_f]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("value methods cannot be virtual");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "open function `f` is only allowed in class declarations"
    );
}

#[test]
fn overriding_without_the_modifier_is_an_error() {
    let file = file(vec![
        describable(),
        class_decl(
            Open,
            "Shape",
            vec![(false, "name", ty_named("String"))],
            None,
            vec!["Describable"],
            vec![method_expr(
                "describe",
                vec![],
                Some(ty_named("String")),
                var("name"),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a missing `override` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`describe` overrides `Describable.describe` and must be marked `override`"
    );
}

#[test]
fn override_without_overriding_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![method_full(
                true,
                false,
                "m",
                vec![],
                None,
                FunctionBody::Block(block(vec![])),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a spurious `override` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`m` is marked `override` but does not override any method"
    );
}
