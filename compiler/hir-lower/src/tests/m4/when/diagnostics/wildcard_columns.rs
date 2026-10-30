use super::*;

#[test]
fn ignored_empty_column_keeps_the_whole_product_uninhabited() {
    let file = file(vec![
        enum_decl("Empty", vec![], vec![]),
        fun_sig(
            "check",
            vec![],
            vec![(
                "value",
                ty_tuple(vec![ty_named("Empty"), ty_named("Boolean")]),
            )],
            None,
            vec![when_stmt(
                var("value"),
                vec![arm(
                    pat_tuple(vec![pat_wild(), pat_lit(bool_lit(true))], None),
                    None,
                    vec![],
                )],
                None,
            )],
        ),
        fun("main", vec![]),
    ]);
    lower_user(file).expect("an empty head has no witness for any missing tail");
}

#[test]
fn ignored_aggregate_columns_do_not_expand_their_cartesian_product() {
    let names: Vec<_> = (0..32).map(|index| format!("flag{index}")).collect();
    let file = file(vec![
        struct_decl(
            "State",
            names
                .iter()
                .map(|name| (name.as_str(), ty_named("Boolean")))
                .collect(),
        ),
        enum_decl(
            "Event",
            vec![],
            vec![variant_positional(
                "Button",
                vec![ty_named("State"), ty_named("Boolean")],
            )],
        ),
        fun_sig(
            "check",
            vec![],
            vec![("event", ty_named("Event"))],
            None,
            vec![when_stmt(
                var("event"),
                [false, true]
                    .into_iter()
                    .map(|pressed| {
                        arm(
                            pat_pos(
                                &["Button"],
                                vec![pat_wild(), pat_lit(bool_lit(pressed))],
                                None,
                            ),
                            None,
                            vec![],
                        )
                    })
                    .collect(),
                None,
            )],
        ),
        fun("main", vec![]),
    ]);
    lower_user(file).expect("ignored flags do not participate in the decision");
}

#[test]
fn ignored_columns_preserve_missing_tail_correlations_and_witness_order() {
    let file = file(vec![
        struct_decl(
            "State",
            vec![
                ("first", ty_named("Boolean")),
                ("second", ty_named("Boolean")),
            ],
        ),
        fun_sig(
            "check",
            vec![],
            vec![(
                "value",
                ty_tuple(vec![
                    ty_named("State"),
                    ty_named("Boolean"),
                    ty_named("Boolean"),
                ]),
            )],
            None,
            vec![when_stmt(
                var("value"),
                [false, true]
                    .into_iter()
                    .map(|flag| {
                        arm(
                            pat_tuple(
                                vec![pat_wild(), pat_lit(bool_lit(flag)), pat_lit(bool_lit(flag))],
                                None,
                            ),
                            None,
                            vec![],
                        )
                    })
                    .collect(),
                None,
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("equal tail flags do not cover unequal flags");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: missing pattern (State { first: false, second: false }, false, true)"
    );
}
