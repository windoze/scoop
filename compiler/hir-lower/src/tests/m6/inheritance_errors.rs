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
fn base_clause_requires_a_class() {
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
    let errors = lower_user(file).expect_err("a non-class base must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`Describable` is not a class");
}

#[test]
fn interface_list_requires_interfaces() {
    let file = file(vec![
        class_decl(Final, "A", vec![], None, vec![], vec![]),
        class_decl(Open, "B", vec![], None, vec!["A"], vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a non-interface in the list must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`A` is not an interface");
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
        "property `x` of class `B` shadows a property of base class `A`"
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
        "constructor of class `A` takes exactly 1 argument, but 2 were supplied"
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
        "argument for constructor property `x` of class `A` must be of type Int, found String"
    );
}

// --- negative: override and implementation ---

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
            .find_map(|(_, function)| (function.name == name).then_some(function.method))
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

#[test]
fn unimplemented_interface_method_is_an_error() {
    let file = file(vec![
        describable(),
        class_decl(Final, "C", vec![], None, vec!["Describable"], vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an unimplemented interface must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "class `C` does not implement interface method `Describable.describe`"
    );
}

#[test]
fn interface_implementation_with_the_wrong_signature_is_an_error() {
    let file = file(vec![
        describable(),
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec!["Describable"],
            vec![method_full(
                true,
                false,
                "describe",
                vec![],
                Some(ty_named("Int")),
                FunctionBody::Expr(Box::new(int_lit(1))),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a signature mismatch must fail");
    assert!(
        errors
            .iter()
            .any(|e| e.message
                == "class `C` does not implement interface method `Describable.describe`"),
        "unexpected diagnostics: {errors:?}"
    );
    assert!(
        errors.iter().any(
            |e| e.message == "`describe` is marked `override` but does not override any method"
        ),
        "unexpected diagnostics: {errors:?}"
    );
}

#[test]
fn abstract_classes_may_leave_interface_methods_unimplemented() {
    let file = file(vec![
        describable(),
        class_decl(Abstract, "C", vec![], None, vec!["Describable"], vec![]),
        fun("main", vec![]),
    ]);
    lower_user(file).expect("abstract classes defer the implementation");
}

// --- negative: abstract and bodies ---

#[test]
fn abstract_class_instantiation_is_an_error() {
    let file = file(vec![
        class_decl(
            Abstract,
            "Base",
            vec![],
            None,
            vec![],
            vec![bodyless_method(true, "kind", vec![], Some(ty_named("Int")))],
        ),
        fun("main", vec![stmt(call("Base", vec![]))]),
    ]);
    let errors = lower_user(file).expect_err("instantiating an abstract class must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "abstract class `Base` cannot be instantiated"
    );
}

#[test]
fn class_construction_lowers_to_class_init() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int")), (false, "a", ty_named("Any"))],
            None,
            vec![],
            vec![],
        ),
        fun(
            "main",
            vec![val("c", call("C", vec![int_lit(1), int_lit(2)]))],
        ),
    ]);
    let module = lower_user(file).expect("class construction must lower");
    let main = body_of(&module, "main");
    match &main.statements[0].kind {
        hir::StatementKind::ValDecl { init, .. } => match &init.kind {
            hir::ExprKind::ClassInit {
                application, args, ..
            } => {
                let class_id = module.class_applications[*application].template;
                assert_eq!(module.classes[class_id].name, "C");
                assert!(matches!(
                    module.types[init.ty],
                    hir::Type::Class(found) if found == *application
                ));
                assert_eq!(args.len(), 2);
                // The Int argument crossing into the `Any` property boxes.
                assert!(matches!(args[0].kind, hir::ExprKind::IntLiteral(1)));
                assert!(matches!(args[1].kind, hir::ExprKind::Box(_)));
            }
            other => panic!("expected a ClassInit, found {other:?}"),
        },
        other => panic!("expected a val decl, found {other:?}"),
    }
}

#[test]
fn class_construction_arity_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        fun("main", vec![val("c", call("C", vec![]))]),
    ]);
    let errors = lower_user(file).expect_err("wrong arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "class `C` takes exactly 1 argument, but 0 were supplied"
    );
}

#[test]
fn class_construction_argument_types_are_checked() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![(false, "x", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        fun("main", vec![val("c", call("C", vec![str_lit("s")]))]),
    ]);
    let errors = lower_user(file).expect_err("a wrong argument type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "argument for field `x` of `C` must be of type Int, found String"
    );
}

#[test]
fn abstract_method_outside_an_abstract_class_is_an_error() {
    let file = file(vec![
        class_decl(
            Open,
            "C",
            vec![],
            None,
            vec![],
            vec![bodyless_method(true, "m", vec![], None)],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("misplaced abstract must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "abstract function `m` is only allowed in abstract classes"
    );
}

#[test]
fn interface_method_with_a_body_is_an_error() {
    let file = file(vec![
        interface_decl("I", vec![method("m", vec![], None, vec![])]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a bodied interface method must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "interface method `I.m` must not have a body"
    );
}

#[test]
fn concrete_method_without_a_body_is_an_error() {
    let file = file(vec![
        class_decl(
            Final,
            "C",
            vec![],
            None,
            vec![],
            vec![bodyless_method(false, "m", vec![], None)],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a bodyless concrete method must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "function `m` must have a body");
}

#[test]
fn final_generic_member_functions_are_resolved() {
    let mut generic = method_expr(
        "id",
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    generic.type_params = vec![type_param("T")];
    let file = file(vec![
        class_decl(Final, "C", vec![], None, vec![], vec![generic]),
        fun(
            "main",
            vec![stmt(method_call(
                call("C", vec![]),
                "id",
                vec![str_lit("ok")],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("a final generic method must lower");
    let method = find_fn(&module, "C.id");
    assert_eq!(module.functions[method].type_param_count(), 1);
    assert_eq!(module.functions[method].type_params()[0].name, "T");
    let hir::FunctionGenericity::GenericMethod {
        definition: generic,
        ..
    } = module.functions[method].genericity
    else {
        panic!("C.id generic entity")
    };
    let (_, request) = module
        .generic_method_applications
        .iter()
        .find(|(_, request)| request.method == generic)
        .expect("the call requests an instance");
    assert_eq!(request.method_arguments.to_vec(), [module.string]);
    assert!(matches!(request.owner, hir::GenericMethodOwner::Class(_)));
}
