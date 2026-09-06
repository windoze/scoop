use super::*;

#[test]
fn catch_non_throwable_is_an_error() {
    let ty_span = Span::new(20, 23);
    let catch = catch_clause(
        "e",
        TypeRef {
            kind: TypeRefKind::Named(ident("Int")),
            span: ty_span,
        },
        vec![],
    );
    let file = file(vec![fun("main", vec![try_stmt(vec![], vec![catch], None)])]);
    let errors = lower_user_with_exceptions(file).expect_err("catching an Int must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "catch parameter type Int is not a subtype of Throwable"
    );
    assert_eq!(errors[0].span, Some(ty_span));
}

#[test]
fn unreachable_catch_is_an_error() {
    // A supertype (or the same type) in an earlier catch covers the
    // later one.
    for (earlier, later) in [
        ("Exception", "UnwrapException"),
        ("Throwable", "MyError"),
        ("Exception", "Exception"),
    ] {
        let clause_span = Span::new(30, 40);
        let file = file(vec![
            custom_error(),
            fun(
                "main",
                vec![try_stmt(
                    vec![],
                    vec![
                        catch_clause("e", ty_named(earlier), vec![]),
                        catch_clause_at("e", ty_named(later), vec![], clause_span),
                    ],
                    None,
                )],
            ),
        ]);
        let errors = lower_user_with_exceptions(file).expect_err("the shadowed catch must fail");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(
            errors[0].message,
            format!("unreachable catch block: {later} is already covered by an earlier catch")
        );
        assert_eq!(errors[0].span, Some(clause_span));
    }
}

#[test]
fn catch_local_is_not_visible_after_the_try() {
    let file = file(vec![fun(
        "main",
        vec![
            try_stmt(
                vec![],
                vec![catch_clause("e", ty_named("Exception"), vec![])],
                None,
            ),
            stmt(call("println", vec![var("e")])),
        ],
    )]);
    let errors = lower_user_with_exceptions(file).expect_err("`e` must be out of scope");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "unknown variable `e`; bare enum variants without an exact enum expected type must be qualified as `E.V` or given a type annotation"
    );
}

// --- negative: the core Throwable contract ---

#[test]
fn missing_core_throwable_is_an_error() {
    // A user file alone (no core) has no `Throwable`.
    let errors =
        lower(&[file(vec![fun("main", vec![])])]).expect_err("missing core Throwable must fail");
    assert!(
        errors
            .iter()
            .any(|e| e.message == "scoop.core must define a class `Throwable`" && e.file == 0),
        "{errors:?}"
    );
}

#[test]
fn core_throwable_must_be_a_class() {
    // A `Throwable` declared as another type kind does not count.
    let core = file(vec![
        enum_decl(
            "Option",
            vec!["T"],
            vec![
                variant_positional("Some", vec![ty_named("T")]),
                variant_unit("None"),
            ],
        ),
        struct_decl("Throwable", vec![]),
    ]);
    let errors = lower(&[core, file(vec![fun("main", vec![])])])
        .expect_err("a struct `Throwable` must fail");
    assert!(
        errors
            .iter()
            .any(|e| e.message == "scoop.core must define a class `Throwable`"),
        "{errors:?}"
    );
}

#[test]
fn user_file_throwable_does_not_count() {
    // `Throwable` declared only in the user file is a core
    // configuration error, not a valid setup.
    let mut core = core_file();
    core.declarations
        .retain(|decl| !matches!(decl, Decl::Class(class) if class.name.text == "Throwable"));
    let user = file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Throwable",
            vec![],
            None,
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower(&[core, user]).expect_err("user-file `Throwable` must fail");
    assert!(
        errors
            .iter()
            .any(|e| e.message == "scoop.core must define a class `Throwable`"),
        "{errors:?}"
    );
}
