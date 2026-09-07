use super::super::*;

#[test]
fn explicit_empty_else_is_preserved_in_both_hir_products() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(
                    var("c"),
                    vec![arm(pat_bind("Red"), None, vec![])],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let output = lower_user_output(file).expect("an explicit empty else branch must lower");

    let hir::FunctionKind::User(export_main) = &output.export.functions[output.export.entry].kind
    else {
        panic!("export main body")
    };
    let export_when = export_main
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::When(value) => Some(value),
            _ => None,
        })
        .expect("export when");
    assert!(matches!(
        &export_when.fallback,
        hir::WhenFallback::Else(body) if body.is_empty()
    ));

    let hir::concrete::FunctionKind::User(local_main) =
        &output.local.functions[output.local.entry].kind
    else {
        panic!("concrete main body")
    };
    let local_when = local_main
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::When(value) => Some(value),
            _ => None,
        })
        .expect("concrete when");
    assert!(matches!(
        &local_when.fallback,
        hir::concrete::WhenFallback::Else(body) if body.is_empty()
    ));
}

#[test]
fn recursive_matrix_combines_nested_enum_tuple_struct_boolean_and_unit_rows() {
    let flags = ty_named("Flags");
    let item_payload = ty_tuple(vec![flags.clone(), ty_named("Unit")]);
    let nested_pattern = |flag| {
        pat_pos(
            &["Item"],
            vec![pat_tuple(
                vec![
                    pat_pos(&["Flags"], vec![pat_lit(bool_lit(flag)), pat_wild()], None),
                    pat_lit(unit_lit()),
                ],
                None,
            )],
            None,
        )
    };
    let file = file(vec![
        struct_decl(
            "Flags",
            vec![
                ("first", ty_named("Boolean")),
                ("second", ty_named("Boolean")),
            ],
        ),
        enum_decl(
            "Nested",
            vec![],
            vec![
                variant_positional("Item", vec![item_payload]),
                variant_unit("Empty"),
            ],
        ),
        enum_decl(
            "BooleanPayload",
            vec![],
            vec![variant_positional("Value", vec![ty_named("Boolean")])],
        ),
        fun_sig(
            "checkNested",
            vec![],
            vec![("value", ty_named("Nested"))],
            None,
            vec![when_stmt(
                var("value"),
                vec![
                    arm(nested_pattern(false), None, vec![]),
                    arm(nested_pattern(true), None, vec![]),
                    arm(pat_bind("Empty"), None, vec![]),
                ],
                None,
            )],
        ),
        fun_sig(
            "checkBooleanPayload",
            vec![],
            vec![("value", ty_named("BooleanPayload"))],
            None,
            vec![when_stmt(
                var("value"),
                vec![
                    arm(
                        pat_pos(&["Value"], vec![pat_lit(bool_lit(false))], None),
                        None,
                        vec![],
                    ),
                    arm(
                        pat_pos(&["Value"], vec![pat_lit(bool_lit(true))], None),
                        None,
                        vec![],
                    ),
                ],
                None,
            )],
        ),
        fun_sig(
            "checkTuple",
            vec![],
            vec![(
                "value",
                ty_tuple(vec![ty_named("Boolean"), ty_named("Boolean")]),
            )],
            None,
            vec![when_stmt(
                var("value"),
                vec![
                    arm(
                        pat_tuple(vec![pat_lit(bool_lit(false)), pat_wild()], None),
                        None,
                        vec![],
                    ),
                    arm(
                        pat_tuple(vec![pat_lit(bool_lit(true)), pat_wild()], None),
                        None,
                        vec![],
                    ),
                ],
                None,
            )],
        ),
        fun_sig(
            "checkIrrefutable",
            vec![],
            vec![("value", ty_named("Flags"))],
            None,
            vec![when_stmt(
                var("value"),
                vec![arm(
                    pat_pos(&["Flags"], vec![pat_wild(), pat_wild()], None),
                    None,
                    vec![],
                )],
                None,
            )],
        ),
        fun("main", vec![]),
    ]);
    let output = lower_user_output(file).expect("recursive constructor matrices are exhaustive");

    let proof_for = |name: &str| {
        let function = output
            .export
            .functions
            .iter()
            .map(|(_, function)| function)
            .find(|function| function.name == name)
            .expect("test function");
        let hir::FunctionKind::User(body) = &function.kind else {
            panic!("test function body")
        };
        body.statements
            .iter()
            .find_map(|statement| match &statement.kind {
                hir::StatementKind::When(when) => Some(&when.fallback),
                _ => None,
            })
            .expect("checked when")
    };
    assert!(matches!(
        proof_for("checkNested"),
        hir::WhenFallback::Impossible(hir::ExhaustivenessProof::EnumPatternMatrix { .. })
    ));
    assert!(matches!(
        proof_for("checkBooleanPayload"),
        hir::WhenFallback::Impossible(hir::ExhaustivenessProof::EnumPatternMatrix { .. })
    ));
    assert!(matches!(
        proof_for("checkTuple"),
        hir::WhenFallback::Impossible(hir::ExhaustivenessProof::PatternMatrix { .. })
    ));
    assert!(matches!(
        proof_for("checkIrrefutable"),
        hir::WhenFallback::Impossible(hir::ExhaustivenessProof::IrrefutableArm { .. })
    ));

    let concrete_proof_for = |name: &str| {
        let function = output
            .local
            .functions
            .iter()
            .map(|(_, function)| function)
            .find(|function| function.name == name)
            .expect("concrete test function");
        let hir::concrete::FunctionKind::User(body) = &function.kind else {
            panic!("concrete test function body")
        };
        body.statements
            .iter()
            .find_map(|statement| match &statement.kind {
                hir::concrete::StatementKind::When(when) => Some(&when.fallback),
                _ => None,
            })
            .expect("concrete checked when")
    };
    assert!(matches!(
        concrete_proof_for("checkNested"),
        hir::concrete::WhenFallback::Impossible(
            hir::concrete::ExhaustivenessProof::EnumPatternMatrix { .. }
        )
    ));
    assert!(matches!(
        concrete_proof_for("checkTuple"),
        hir::concrete::WhenFallback::Impossible(
            hir::concrete::ExhaustivenessProof::PatternMatrix { .. }
        )
    ));
    assert!(matches!(
        concrete_proof_for("checkIrrefutable"),
        hir::concrete::WhenFallback::Impossible(
            hir::concrete::ExhaustivenessProof::IrrefutableArm { .. }
        )
    ));
}

#[test]
fn finite_nested_values_empty_payloads_and_reference_cycles_remain_valid() {
    let nested_box = |value| {
        pat_pos(
            &["Box"],
            vec![pat_pos(&["Box"], vec![pat_lit(bool_lit(value))], None)],
            None,
        )
    };
    let file = file(vec![
        enum_decl("Never", vec![], vec![]),
        enum_decl(
            "MaybeNever",
            vec![],
            vec![
                variant_positional("ImpossiblePayload", vec![ty_named("Never")]),
                variant_unit("OnlyInhabited"),
            ],
        ),
        enum_decl(
            "Links",
            vec![],
            vec![
                variant_positional("Link", vec![ty_named("Node")]),
                variant_unit("NoLink"),
            ],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Node",
            vec![(false, "next", ty_named("Links"))],
            None,
            vec![],
            vec![],
        ),
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        generic_struct_decl("Phantom", vec!["T"], vec![]),
        generic_struct_decl(
            "PhantomForward",
            vec!["T"],
            vec![("marker", ty_generic("Phantom", vec![ty_named("T")]))],
        ),
        struct_decl(
            "SelfMarker",
            vec![(
                "marker",
                ty_generic("Phantom", vec![ty_named("SelfMarker")]),
            )],
        ),
        struct_decl(
            "TransitiveSelfMarker",
            vec![(
                "marker",
                ty_generic("PhantomForward", vec![ty_named("TransitiveSelfMarker")]),
            )],
        ),
        struct_decl(
            "PointerCycle",
            vec![("next", ty_generic("Ptr", vec![ty_named("PointerCycle")]))],
        ),
        fun_sig(
            "checkEmptyPayload",
            vec![],
            vec![("value", ty_named("MaybeNever"))],
            None,
            vec![when_stmt(
                var("value"),
                vec![arm(pat_bind("OnlyInhabited"), None, vec![])],
                None,
            )],
        ),
        fun_sig(
            "checkReferenceBoundary",
            vec![],
            vec![("value", ty_named("Links"))],
            None,
            vec![when_stmt(
                var("value"),
                vec![
                    arm(
                        pat_pos(&["Link"], vec![pat_bind("node")], None),
                        None,
                        vec![],
                    ),
                    arm(pat_bind("NoLink"), None, vec![]),
                ],
                None,
            )],
        ),
        fun_sig(
            "checkFiniteNesting",
            vec![],
            vec![(
                "value",
                ty_generic("Box", vec![ty_generic("Box", vec![ty_named("Boolean")])]),
            )],
            None,
            vec![when_stmt(
                var("value"),
                vec![
                    arm(nested_box(false), None, vec![]),
                    arm(nested_box(true), None, vec![]),
                ],
                None,
            )],
        ),
        fun_sig(
            "checkPhantomSelfArgument",
            vec![],
            vec![("value", ty_named("SelfMarker"))],
            None,
            vec![when_stmt(
                var("value"),
                vec![arm(
                    pat_pos(&["SelfMarker"], vec![pat_wild()], None),
                    None,
                    vec![],
                )],
                None,
            )],
        ),
        fun("main", vec![]),
    ]);

    lower_user_output(file).expect(
        "only genuine by-value declaration cycles are rejected; empty payloads stay uninhabited",
    );
}
