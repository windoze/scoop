use super::*;

#[test]
fn generic_method_applications_keep_owner_and_method_arguments_separate() {
    let mut choose = method_expr(
        "choose",
        vec![("value", ty_named("U"))],
        Some(ty_named("U")),
        var("value"),
    );
    choose.type_params = vec![type_param("U")];
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: Some(Box::new(var("box"))),
        name: ident("choose"),
        span: sp(),
    };
    let output = lower_user_output(file(vec![
        generic_class(
            "Box",
            vec![type_param("T")],
            vec![(false, "value", ty_named("T"))],
            vec![choose],
        ),
        fun(
            "main",
            vec![
                val("box", call("Box", vec![str_lit("owner")])),
                val(
                    "number",
                    typed_method_call(
                        var("box"),
                        "choose",
                        vec![ty_named("Int")],
                        vec![int_lit(42)],
                    ),
                ),
                val_ty(
                    "stringChoice",
                    Some(ty_function(
                        false,
                        vec![ty_named("String")],
                        ty_named("String"),
                    )),
                    reference,
                ),
            ],
        ),
    ]))
    .expect("generic method calls and references must have exact applications");

    let method = output
        .export
        .functions
        .iter()
        .find(|(_, function)| function.name == "Box.choose")
        .expect("generic method declaration")
        .0;
    let hir::FunctionGenericity::GenericMethod {
        definition,
        owner_parameters,
        method_parameters,
    } = &output.export.functions[method].genericity
    else {
        panic!("Box.choose must have a distinct generic-method identity")
    };
    assert_eq!(owner_parameters.len(), 1);
    assert_eq!(method_parameters.len(), 1);
    assert_ne!(
        owner_parameters[0].id.identity_raw(),
        method_parameters.iter().next().unwrap().id.identity_raw(),
        "owner and method parameters have distinct semantic identities"
    );
    assert_eq!(owner_parameters[0].id.into_raw(), 0);
    assert_eq!(method_parameters.iter().next().unwrap().id.into_raw(), 1);

    let applications = output
        .export
        .generic_method_applications
        .iter()
        .filter(|(_, application)| application.method == *definition)
        .map(|(_, application)| application)
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 2);
    for application in &applications {
        let hir::GenericMethodOwner::Class(owner) = application.owner else {
            panic!("Box.choose must retain its exact class application")
        };
        assert_eq!(
            output.export.class_applications[owner].arguments,
            vec![output.export.string]
        );
    }
    assert!(
        applications
            .iter()
            .any(|application| application.method_arguments.to_vec() == [output.export.int])
    );
    assert!(
        applications
            .iter()
            .any(|application| application.method_arguments.to_vec() == [output.export.string])
    );

    let concrete_origin =
        hir::concrete::GenericMethodOriginId::from_raw(definition.into_raw().into_u32());
    let concrete_methods = output
        .local
        .functions
        .iter()
        .filter_map(|(_, function)| match &function.origin {
            hir::concrete::FunctionOrigin::Method(hir::concrete::MethodOrigin {
                owner: hir::concrete::MethodOwner::Class(owner),
                specialization:
                    hir::concrete::MethodSpecialization::Generic {
                        origin,
                        method_arguments,
                        ..
                    },
            }) if *origin == concrete_origin => Some((*owner, method_arguments.to_vec())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(concrete_methods.len(), 2);
    for (owner, _) in &concrete_methods {
        assert_eq!(
            output.local.classes[*owner].type_arguments,
            vec![output.local.string]
        );
        assert!(
            output.local.classes[*owner]
                .methods
                .iter()
                .all(|method| output.local.functions[*method].name != "Box.choose"),
            "generic methods are direct applications and never dispatch-table members"
        );
    }
    assert!(
        concrete_methods
            .iter()
            .any(|(_, arguments)| arguments == &[output.local.int])
    );
    assert!(
        concrete_methods
            .iter()
            .any(|(_, arguments)| arguments == &[output.local.string])
    );
}

#[test]
fn parameterized_method_families_use_one_source_identity_domain_for_symbols() {
    let ordinary = method_expr(
        "keep",
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    let mut generic = method_expr(
        "keep",
        vec![("value", ty_named("U")), ("marker", ty_named("Int"))],
        Some(ty_named("U")),
        var("value"),
    );
    generic.type_params = vec![type_param("U")];
    let output = lower_user_output(file(vec![
        generic_class(
            "Host",
            vec![type_param("T")],
            vec![(false, "value", ty_named("T"))],
            vec![ordinary, generic],
        ),
        fun(
            "main",
            vec![
                val("host", call("Host", vec![str_lit("owner")])),
                stmt(method_call(var("host"), "keep", vec![str_lit("ordinary")])),
                stmt(method_call(
                    var("host"),
                    "keep",
                    vec![int_lit(42), int_lit(0)],
                )),
            ],
        ),
    ]))
    .expect("both parameterized method families have exact concrete identities");

    let symbols = output
        .local
        .functions
        .iter()
        .filter_map(|(_, function)| {
            if function.name != "Host.keep" {
                return None;
            }
            let hir::concrete::FunctionOrigin::Method(origin) = &function.origin else {
                return None;
            };
            match &origin.specialization {
                hir::concrete::MethodSpecialization::OwnerParameterized { symbol, .. }
                | hir::concrete::MethodSpecialization::Generic { symbol, .. } => Some(*symbol),
                hir::concrete::MethodSpecialization::Plain => None,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(symbols.len(), 2);
    let discriminators = symbols
        .iter()
        .map(|symbol| match symbol {
            hir::concrete::InstanceSymbol::Overloaded { discriminator } => *discriminator,
            hir::concrete::InstanceSymbol::Unique => {
                panic!("same-name parameterized declarations require explicit symbol identities")
            }
        })
        .collect::<Vec<_>>();
    assert_ne!(discriminators[0], discriminators[1]);
}

#[test]
fn generic_method_bounds_are_checked_with_the_method_argument_group() {
    let mut accept = method_expr(
        "accept",
        vec![("value", ty_named("U"))],
        Some(ty_named("U")),
        var("value"),
    );
    accept.type_params = vec![upper("U", ty_named("Marker"))];
    let host = generic_class(
        "Host",
        vec![type_param("T")],
        vec![(false, "value", ty_named("T"))],
        vec![accept],
    );

    lower_user(file(vec![
        interface_decl("Marker", vec![]),
        struct_decl_full("Marked", vec![], vec!["Marker"], vec![]),
        host.clone(),
        fun(
            "main",
            vec![stmt(method_call(
                call("Host", vec![int_lit(1)]),
                "accept",
                vec![struct_init("Marked", vec![])],
            ))],
        ),
    ]))
    .expect("the method argument satisfies its exact upper bound");

    let errors = lower_user(file(vec![
        interface_decl("Marker", vec![]),
        host,
        fun(
            "main",
            vec![stmt(method_call(
                call("Host", vec![int_lit(1)]),
                "accept",
                vec![int_lit(2)],
            ))],
        ),
    ]))
    .expect_err("the owner argument must not be used in place of the method argument");
    assert!(errors.iter().any(|error| {
        error.message
            == "type argument `Int` for `U` of function `Host.accept` must satisfy interface upper bound `Marker`"
    }));
}

#[test]
fn generic_callable_recursion_requires_an_identity_argument_mapping() {
    let direct = fun_sig(
        "direct",
        vec!["T"],
        vec![("value", ty_named("T"))],
        None,
        vec![stmt(typed_call(
            "direct",
            vec![ty_named("T")],
            vec![var("value")],
        ))],
    );
    let first = fun_sig(
        "first",
        vec!["A", "B"],
        vec![("a", ty_named("A")), ("b", ty_named("B"))],
        None,
        vec![stmt(typed_call(
            "second",
            vec![ty_named("B"), ty_named("A")],
            vec![var("b"), var("a")],
        ))],
    );
    let second = fun_sig(
        "second",
        vec!["X", "Y"],
        vec![("x", ty_named("X")), ("y", ty_named("Y"))],
        None,
        vec![stmt(typed_call(
            "first",
            vec![ty_named("Y"), ty_named("X")],
            vec![var("y"), var("x")],
        ))],
    );
    lower_user(file(vec![direct, first, second, fun("main", vec![])]))
        .expect("identity recursion and an identity-composing parameter permutation terminate");

    let growing = fun_sig(
        "growing",
        vec!["T"],
        vec![("value", ty_named("T"))],
        None,
        vec![stmt(typed_call(
            "growing",
            vec![ty_nullable(ty_named("T"))],
            vec![some(var("value"))],
        ))],
    );
    let errors = lower_user(file(vec![growing, fun("main", vec![])]))
        .expect_err("a recursive application that grows its argument is polymorphic recursion");
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("polymorphic recursion is not supported")
            && error.message.contains("growing")
            && error
                .message
                .contains("changes its complete type-argument mapping")
    }));
}

#[test]
fn generic_method_polymorphic_recursion_checks_both_argument_groups() {
    let mut grow = method(
        "grow",
        vec![("value", ty_named("U"))],
        None,
        vec![stmt(typed_method_call(
            this_expr(),
            "grow",
            vec![ty_nullable(ty_named("U"))],
            vec![some(var("value"))],
        ))],
    );
    grow.type_params = vec![type_param("U")];
    let host = generic_struct_decl_full(
        "Host",
        vec!["T"],
        vec![("owner", ty_named("T"))],
        vec![],
        vec![grow],
    );
    let errors = lower_user(file(vec![host, fun("main", vec![])]))
        .expect_err("changing the method suffix in a recursive cycle must fail");
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("polymorphic recursion is not supported")
            && error.message.contains("Host.grow")
    }));
}
