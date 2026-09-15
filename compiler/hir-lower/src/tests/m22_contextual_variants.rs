use super::*;

fn function_body<'module>(module: &'module hir::Module, name: &str) -> &'module hir::Body {
    module
        .functions
        .iter()
        .find_map(|(_, function)| {
            if function.name != name {
                return None;
            }
            match &function.kind {
                hir::FunctionKind::User(body) => Some(body),
                _ => None,
            }
        })
        .unwrap_or_else(|| panic!("user function `{name}` must exist"))
}

fn assert_variant(module: &hir::Module, expression: &hir::Expr, owner: &str, name: &str) {
    let hir::ExprKind::VariantConstruct { variant, .. } = &expression.kind else {
        panic!("expected a variant construction, found {expression:?}");
    };
    let application = variant.application();
    let enumeration = module.enum_applications[application].template;
    assert_eq!(module.enums[enumeration].name, owner);
    assert_eq!(
        module.enums[enumeration].variants[variant.local_index() as usize].name,
        name
    );
}

fn state_decl() -> Decl {
    enum_decl(
        "State",
        Vec::new(),
        vec![
            variant_unit("Ready"),
            variant_positional("Value", vec![ty_named("Int")]),
            variant_positional("Pair", vec![ty_named("Int"), ty_named("Int")]),
        ],
    )
}

#[test]
fn exact_expected_enum_types_bare_variants_in_annotations_returns_and_arguments() {
    let module = lower_user(file(vec![
        state_decl(),
        fun_expr(
            "returned",
            Vec::new(),
            Vec::new(),
            Some(ty_named("State")),
            var("Ready"),
        ),
        fun_expr(
            "pass",
            Vec::new(),
            vec![("value", ty_named("State"))],
            Some(ty_named("State")),
            var("value"),
        ),
        fun(
            "main",
            vec![
                val_ty("ready", Some(ty_named("State")), var("Ready")),
                val_ty(
                    "payload",
                    Some(ty_named("State")),
                    call("Value", vec![int_lit(1)]),
                ),
                val(
                    "argument",
                    call("pass", vec![call("Value", vec![int_lit(2)])]),
                ),
            ],
        ),
    ]))
    .expect("every direct expected-type source must resolve a bare variant");

    let main = function_body(&module, "main");
    assert_variant(&module, local_init(main, "ready"), "State", "Ready");
    assert_variant(&module, local_init(main, "payload"), "State", "Value");
    assert_variant(
        &module,
        return_value(&function_body(&module, "returned").statements),
        "State",
        "Ready",
    );
    let dump = hir::dump(&module);
    assert_eq!(
        dump.matches("VariantConstruct State.Value").count(),
        2,
        "{dump}"
    );
}

#[test]
fn generic_fixed_points_work_in_both_source_orders_and_equality_keeps_effect_order() {
    let module = lower_user(file(vec![
        enum_decl(
            "Event",
            Vec::new(),
            vec![variant_positional("Value", vec![ty_named("Int")])],
        ),
        fun_expr(
            "choose",
            vec!["T"],
            vec![("first", ty_named("T")), ("second", ty_named("T"))],
            Some(ty_named("T")),
            var("first"),
        ),
        fun_expr(
            "leftEffect",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(7),
        ),
        fun_expr(
            "rightEffect",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(7),
        ),
        fun(
            "main",
            vec![
                val(
                    "forward",
                    call(
                        "choose",
                        vec![
                            call("Value", vec![int_lit(1)]),
                            call("Event.Value", vec![int_lit(2)]),
                        ],
                    ),
                ),
                val(
                    "reverse",
                    call(
                        "choose",
                        vec![
                            call("Event.Value", vec![int_lit(3)]),
                            call("Value", vec![int_lit(4)]),
                        ],
                    ),
                ),
                val(
                    "same",
                    binary(
                        BinOp::Eq,
                        call("Value", vec![call("leftEffect", Vec::new())]),
                        call("Event.Value", vec![call("rightEffect", Vec::new())]),
                    ),
                ),
            ],
        ),
    ]))
    .expect("a qualified peer must fix a postponed bare variant in either order");

    let main = function_body(&module, "main");
    assert_eq!(
        hir::type_name(&module, local_init(main, "forward").ty),
        "Event"
    );
    assert_eq!(
        hir::type_name(&module, local_init(main, "reverse").ty),
        "Event"
    );
    let dump = hir::dump(&module);
    assert_eq!(dump.matches("Call leftEffect").count(), 1, "{dump}");
    assert_eq!(dump.matches("Call rightEffect").count(), 1, "{dump}");
    assert!(
        dump.find("Call leftEffect").expect("left effect")
            < dump.find("Call rightEffect").expect("right effect"),
        "{dump}"
    );
}

#[test]
fn when_branch_contextuality_is_classified_with_pattern_bindings_in_scope() {
    let module = lower_user(file(vec![
        enum_decl(
            "Input",
            Vec::new(),
            vec![
                variant_positional("Number", vec![ty_named("Int")]),
                variant_unit("Empty"),
            ],
        ),
        fun(
            "main",
            vec![
                val("input", call("Input.Number", vec![int_lit(1)])),
                val(
                    "result",
                    Expr::When(Box::new(ast::When {
                        subject: var("input"),
                        arms: vec![
                            arm(pat_bind("Empty"), None, vec![stmt(var("None"))]),
                            arm(
                                pat_pos(&["Number"], vec![pat_bind("value")], None),
                                None,
                                vec![stmt(call("Some", vec![var("value")]))],
                            ),
                        ],
                        else_body: None,
                        span: sp(),
                    })),
                ),
                val_ty(
                    "expected",
                    Some(ty_nullable(ty_named("Int"))),
                    Expr::When(Box::new(ast::When {
                        subject: var("input"),
                        arms: vec![
                            arm(pat_bind("Empty"), None, vec![stmt(var("None"))]),
                            arm(
                                pat_pos(&["Number"], vec![pat_bind("value")], None),
                                None,
                                vec![stmt(call("Some", vec![var("value")]))],
                            ),
                        ],
                        else_body: None,
                        span: sp(),
                    })),
                ),
            ],
        ),
    ]))
    .expect("a pattern-bound payload must seed its peer contextual unit variant");

    assert_eq!(
        hir::type_name(
            &module,
            local_init(function_body(&module, "main"), "result").ty
        ),
        "Option<Int>"
    );
    assert_eq!(
        hir::type_name(
            &module,
            local_init(function_body(&module, "main"), "expected").ty
        ),
        "Option<Int>"
    );
}

#[test]
fn inapplicable_function_struct_and_class_layers_fall_through_to_contextual_variants() {
    let module = lower_user(file(vec![
        enum_decl(
            "Target",
            Vec::new(),
            vec![
                variant_positional("FromFunction", vec![ty_named("Int")]),
                variant_positional("FromStruct", vec![ty_named("Int")]),
                variant_positional("FromClass", vec![ty_named("Int")]),
                variant_positional("FromLocal", vec![ty_named("Int")]),
            ],
        ),
        fun_expr(
            "FromFunction",
            Vec::new(),
            vec![("value", ty_named("String"))],
            Some(ty_named("String")),
            var("value"),
        ),
        struct_decl("FromStruct", vec![("value", ty_named("String"))]),
        class_decl(
            ast::ClassModifier::Final,
            "FromClass",
            vec![(false, "value", ty_named("String"))],
            None,
            Vec::new(),
            Vec::new(),
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "function",
                    Some(ty_named("Target")),
                    call("FromFunction", vec![int_lit(1)]),
                ),
                val_ty(
                    "structure",
                    Some(ty_named("Target")),
                    call("FromStruct", vec![int_lit(2)]),
                ),
                val_ty(
                    "class",
                    Some(ty_named("Target")),
                    call("FromClass", vec![int_lit(3)]),
                ),
                val("FromLocal", int_lit(99)),
                val_ty(
                    "local",
                    Some(ty_named("Target")),
                    call("FromLocal", vec![int_lit(4)]),
                ),
            ],
        ),
    ]))
    .expect("inapplicable ordinary layers and a plain local must permit contextual lookup");

    let main = function_body(&module, "main");
    for (local, variant) in [
        ("function", "FromFunction"),
        ("structure", "FromStruct"),
        ("class", "FromClass"),
        ("local", "FromLocal"),
    ] {
        assert_variant(&module, local_init(main, local), "Target", variant);
    }
}

#[test]
fn applicable_ordinary_layers_and_callable_locals_keep_their_priority() {
    let callable = Expr::Lambda {
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
    let errors = lower_user(file(vec![
        state_decl(),
        fun_expr(
            "Value",
            Vec::new(),
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "ordinary",
                    Some(ty_named("State")),
                    call("Value", vec![int_lit(1)]),
                ),
                block_stmt(vec![
                    val_ty(
                        "Value",
                        Some(ty_function(false, vec![ty_named("Int")], ty_named("Int"))),
                        callable,
                    ),
                    val_ty(
                        "shadowed",
                        Some(ty_named("State")),
                        call("Value", vec![int_lit(2)]),
                    ),
                ]),
            ],
        ),
    ]))
    .expect_err("an applicable function and callable local must not become enum constructors");

    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(
        errors.iter().all(|error| {
            error
                .message
                .contains("initializer of `ordinary` must be of type State, found Int")
                || error
                    .message
                    .contains("initializer of `shadowed` must be of type State, found Int")
        }),
        "{errors:?}"
    );
}

#[test]
fn prelude_and_contextual_variants_filter_shape_before_layer_selection() {
    let module = lower_user(file(vec![
        enum_decl(
            "LocalOption",
            Vec::new(),
            vec![
                variant_positional("Some", vec![ty_named("Int"), ty_named("Int")]),
                variant_unit("None"),
            ],
        ),
        enum_decl(
            "ReverseShape",
            Vec::new(),
            vec![
                variant_unit("Some"),
                variant_positional("None", vec![ty_named("Int")]),
            ],
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "localSome",
                    Some(ty_named("LocalOption")),
                    call("Some", vec![int_lit(1), int_lit(2)]),
                ),
                val_ty("localNone", Some(ty_named("LocalOption")), var("None")),
                val_ty("unitSome", Some(ty_named("ReverseShape")), var("Some")),
                val_ty(
                    "payloadNone",
                    Some(ty_named("ReverseShape")),
                    call("None", vec![int_lit(3)]),
                ),
            ],
        ),
    ]))
    .expect("an inapplicable prelude shape must not block an exact contextual target");

    let main = function_body(&module, "main");
    for (local, owner, variant) in [
        ("localSome", "LocalOption", "Some"),
        ("localNone", "LocalOption", "None"),
        ("unitSome", "ReverseShape", "Some"),
        ("payloadNone", "ReverseShape", "None"),
    ] {
        assert_variant(&module, local_init(main, local), owner, variant);
    }
}

#[test]
fn contextual_failures_report_exact_enum_shape_and_missing_targets() {
    let errors = lower_user(file(vec![
        state_decl(),
        fun_expr(
            "Missing",
            Vec::new(),
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        fun(
            "main",
            vec![
                val_ty("missingValue", Some(ty_named("State")), var("Missing")),
                val_ty(
                    "missingCall",
                    Some(ty_named("State")),
                    call("Some", vec![int_lit(1), int_lit(2)]),
                ),
                val_ty("payloadAsValue", Some(ty_named("State")), var("Value")),
                val_ty(
                    "unitAsCall",
                    Some(ty_named("State")),
                    call("Ready", Vec::new()),
                ),
                val_ty(
                    "wrongArity",
                    Some(ty_named("State")),
                    call("Pair", vec![int_lit(1)]),
                ),
                val_ty(
                    "ordinaryFailure",
                    Some(ty_named("State")),
                    call("Missing", Vec::new()),
                ),
            ],
        ),
    ]))
    .expect_err("invalid contextual variants must have focused diagnostics");

    let messages = errors
        .iter()
        .map(|error| error.message.as_str())
        .collect::<Vec<_>>();
    assert!(
        messages.contains(
            &"function `Missing` is not a value; use `::Missing` to create a callable reference"
        ),
        "{errors:?}"
    );
    assert!(
        messages.contains(&"enum `State` has no variant `Some`"),
        "{errors:?}"
    );
    assert!(
        messages.contains(
            &"variant `Value` of `State` takes arguments; use `Value(...)` to construct it"
        ),
        "{errors:?}"
    );
    assert!(messages.contains(&"unit variant `Ready` of `State` does not take arguments; use `Ready` without parentheses"), "{errors:?}");
    assert!(
        messages.iter().any(|message| message.contains(
            "variant State.Pair(_1: Int, _2: Int) — expects 2 argument(s), but 1 were supplied"
        )),
        "{errors:?}"
    );
    assert!(
        messages.iter().any(|message| message
            .contains("fun Missing(value: Int): Int — expects 1 argument(s), but 0 were supplied")),
        "{errors:?}"
    );
}

#[test]
fn non_enum_or_absent_expected_types_never_trigger_a_global_variant_search() {
    let errors = lower_user(file(vec![
        state_decl(),
        fun(
            "main",
            vec![
                val("unit", var("Ready")),
                val("payload", call("Value", vec![int_lit(1)])),
                val_ty("wide", Some(ty_named("Any")), var("Ready")),
            ],
        ),
    ]))
    .expect_err("bare variants require one exact enum expected type");

    assert_eq!(errors.len(), 3, "{errors:?}");
    assert!(
        errors
            .iter()
            .all(|error| error.message.contains("qualified as `E.V`"))
    );
}

#[test]
fn candidate_independent_unknown_names_keep_their_focused_diagnostic() {
    let errors = lower_user(file(vec![
        fun_expr(
            "consume",
            Vec::new(),
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        fun_expr(
            "consume",
            Vec::new(),
            vec![("value", ty_named("String"))],
            Some(ty_named("String")),
            var("value"),
        ),
        fun(
            "main",
            vec![val("result", call("consume", vec![var("missing")]))],
        ),
    ]))
    .expect_err("an unknown argument must fail before overload-specific wrapping");

    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        errors[0].message,
        "unknown variable `missing`; bare enum variants without an exact enum expected type must be qualified as `E.V` or given a type annotation"
    );
}

#[test]
fn checked_variant_references_reject_wrong_owners_and_indices() {
    let module = lower_user(file(vec![
        enum_decl(
            "Maybe",
            Vec::new(),
            vec![
                variant_positional("Present", vec![ty_named("Int")]),
                variant_unit("Absent"),
            ],
        ),
        enum_decl("Other", Vec::new(), vec![variant_unit("Absent")]),
        fun("main", Vec::new()),
    ]))
    .expect("the identity fixture must lower");
    let maybe = module
        .enums
        .iter()
        .find_map(|(id, item)| (item.name == "Maybe").then_some(id))
        .expect("Maybe enum");
    let other = module
        .enums
        .iter()
        .find_map(|(id, item)| (item.name == "Other").then_some(id))
        .expect("Other enum");
    let present = hir::EnumVariantRef::checked(&module.enums, maybe, 0).expect("present ref");
    let absent = hir::EnumVariantRef::checked(&module.enums, maybe, 1).expect("absent ref");
    let present_payload =
        hir::EnumVariantFieldRef::checked(&module.enums, present, 0).expect("present payload ref");
    let other_absent =
        hir::EnumVariantRef::checked(&module.enums, other, 0).expect("other absent ref");

    assert!(hir::EnumVariantRef::checked(&module.enums, maybe, 2).is_none());
    assert!(
        hir::OptionCore::checked(&module.enums, &module.types, present_payload, other_absent)
            .is_none()
    );
    assert!(hir::EnumVariantFieldRef::checked(&module.enums, absent, 0).is_none());
    assert!(
        hir::OptionCore::checked(&module.enums, &module.types, present_payload, absent).is_none()
    );

    let checked = module.core_protocols.option;
    assert_eq!(module.enums[checked.enumeration()].name, "Option");
    assert_eq!(checked.some_payload().variant(), checked.some());
    assert_eq!(checked.some_payload().local_index(), 0);
    assert_eq!(
        module.enums[checked.enumeration()].variants[checked.some().local_index() as usize].name,
        "Some"
    );
    assert_eq!(
        module.enums[checked.enumeration()].variants[checked.none().local_index() as usize].name,
        "None"
    );
}
