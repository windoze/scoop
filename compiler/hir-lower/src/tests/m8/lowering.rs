use super::*;

#[test]
fn try_catch_finally_golden() {
    let file = file(vec![
        custom_error(),
        fun_sig(
            "read",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![
                throw_stmt(call("UnwrapException", vec![])),
                ret(Some(int_lit(1))),
            ],
        ),
        fun(
            "main",
            vec![
                try_stmt(
                    vec![
                        stmt(call("read", vec![])),
                        stmt(call("println", vec![str_lit("unreachable")])),
                    ],
                    vec![
                        catch_clause(
                            "e",
                            ty_named("UnwrapException"),
                            vec![stmt(call("println", vec![str_lit("caught unwrap")]))],
                        ),
                        catch_clause(
                            "e",
                            ty_named("Exception"),
                            vec![stmt(call("println", vec![str_lit("caught other")]))],
                        ),
                    ],
                    Some(vec![stmt(call("println", vec![str_lit("finally")]))]),
                ),
                try_stmt(
                    vec![throw_stmt(call("MyError", vec![int_lit(42)]))],
                    vec![catch_clause(
                        "e",
                        ty_named("MyError"),
                        vec![stmt(call("println", vec![field(var("e"), "code")]))],
                    )],
                    Some(vec![stmt(call("println", vec![str_lit("done")]))]),
                ),
            ],
        ),
    ]);
    let module = lower_user_with_exceptions(file).expect("the exception program must lower");
    let expected = include_str!("snapshots/try_catch_finally_golden.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}

// --- positive: structure and scoping ---

/// The catch local is an immutable local of the clause type, visible
/// in its own clause body only; sibling clauses may reuse the name.
#[test]
fn catch_local_structure() {
    let file = file(vec![fun(
        "main",
        vec![try_stmt(
            vec![throw_stmt(call("Throwable", vec![]))],
            vec![
                catch_clause("e", ty_named("UnwrapException"), vec![]),
                catch_clause("e", ty_named("Exception"), vec![]),
            ],
            None,
        )],
    )]);
    let module = lower_user_with_exceptions(file).expect("the try must lower");
    let main = &module.functions[module.entry()];
    let hir::FunctionKind::User(body) = &main.kind else {
        panic!("main has a user body");
    };
    let hir::StatementKind::Try(try_) = &body.statements[0].kind else {
        panic!("expected a try statement");
    };
    assert_eq!(try_.catches.len(), 2);
    let first = &try_.catches[0];
    assert_eq!(body.locals[first.local].name, "e");
    assert!(!body.locals[first.local].mutable);
    assert_eq!(body.locals[first.local].ty, first.ty);
    assert!(matches!(
        module.types[first.ty],
        hir::Type::Class(application)
            if module.classes[module.nominal_identities.class_id(module.class_applications[application].template).expect("an application retains its declaration")].name
                == "UnwrapException"
                && module.class_applications[application].arguments.is_empty()
    ));
}

/// Custom exception classes extend the `open` core `Exception` (the
/// standard M6 inheritance rule — no exception-hierarchy special
/// cases).
#[test]
fn custom_exceptions_extend_open_exception() {
    let file = file(vec![
        custom_error(),
        fun(
            "main",
            vec![
                val_ty(
                    "e",
                    Some(ty_named("Throwable")),
                    call("MyError", vec![int_lit(7)]),
                ),
                throw_stmt(var("e")),
            ],
        ),
    ]);
    lower_user_with_exceptions(file).expect("exception subclasses must lower");
}

/// The built-in exception subclasses are final: inheriting one is an
/// error, exactly like any other final class.
#[test]
fn inheriting_a_final_exception_subclass_is_an_error() {
    let file = file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Bad",
            vec![],
            Some(("UnwrapException", vec![])),
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user_with_exceptions(file).expect_err("inheriting a final class must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "class `UnwrapException` is final and cannot be inherited"
    );
}

#[test]
fn bare_class_supertype_is_classified_as_the_base() {
    let file = file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "E",
            vec![(false, "message", ty_nullable(ty_named("String")))],
            None,
            vec!["Throwable"],
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user_with_exceptions(file).expect("a bare class supertype must lower");
    let (_, class) = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "E")
        .expect("E is declared");
    let base = class.base_class.as_ref().expect("E has a base class");
    assert!(matches!(module.types[*base], hir::Type::Class(application)
        if module.classes[module.nominal_identities.class_id(module.class_applications[application].template).expect("an application retains its declaration")].name == "Throwable"));
    let hir::ClassConstructorKind::Primary {
        base:
            hir::BaseInitialization::Super {
                arguments: delegation,
                ..
            },
        ..
    } = &module.class_constructors[class.constructors[0]].kind
    else {
        panic!("E delegates to Throwable")
    };
    assert!(delegation.args.is_empty());
}

// --- negative: throw ---
