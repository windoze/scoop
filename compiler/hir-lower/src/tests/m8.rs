//! M8 tests: exceptions (milestone8 DESIGN.md 3.2) — `throw` operands
//! and catch parameter types checked against the core `Throwable`
//! class, catch shadowing (unreachable catch is an error, DESIGN.md
//! 5.1), catch-local scoping, and the core contract that `scoop.core`
//! must define a class `Throwable`.
//!
//! The parser-level rules (catch parameters require a type annotation,
//! a `try` needs at least one `catch` or a `finally`, `try` is not an
//! expression) are covered by `scoop-parser`'s M8 tests; the AST
//! cannot represent their violations.

use super::*;

fn custom_error() -> Decl {
    class_decl(
        ast::ClassModifier::Final,
        "MyError",
        vec![(false, "code", ty_named("Int"))],
        Some(("Exception", vec![some(str_lit("my error"))])),
        vec![],
        vec![],
    )
}

// --- positive: golden dump ---

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
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  open class Exception(message: Option<String>)
  class UnwrapException()
  class ClassCastException()
  class ArithmeticException()
  class IndexOutOfBoundsException()
  class MyError(code: Int)
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun read(): Int
    throw
      ClassInit UnwrapException : UnwrapException
    return
      IntLiteral 1 : Int
  fun main(): Unit
    try
      Call read : Int
      Call println : Unit
        StringLiteral \"unreachable\" : Any
    catch e: UnwrapException
      Call println : Unit
        StringLiteral \"caught unwrap\" : Any
    catch e: Exception
      Call println : Unit
        StringLiteral \"caught other\" : Any
    finally
      Call println : Unit
        StringLiteral \"finally\" : Any
    try
      throw
        ClassInit MyError : MyError
          IntLiteral 42 : Int
    catch e: MyError
      Call println : Unit
        Box : Any
          FieldAccess class field 1 : Int
            Local e : MyError
    finally
      Call println : Unit
        StringLiteral \"done\" : Any
  entry main
";
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
    let main = &module.functions[module.entry];
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
        hir::Type::Class(id) if module.classes[id].name == "UnwrapException"
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

/// A bare supertype naming a class (`class E : Throwable`, no
/// constructor parentheses) is an interface-list entry and fails
/// because the name is a class — the Kotlin behavior.
#[test]
fn bare_class_supertype_is_not_an_interface() {
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
    let errors = lower_user_with_exceptions(file).expect_err("a bare class supertype must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`Throwable` is not an interface");
}

// --- negative: throw ---

#[test]
fn throw_non_throwable_is_an_error() {
    let value_span = Span::new(10, 12);
    let value = Expr::IntLiteral {
        value: 42,
        span: value_span,
    };
    let file = file(vec![fun("main", vec![throw_stmt(value)])]);
    let errors = lower_user_with_exceptions(file).expect_err("throwing an Int must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cannot throw value of type Int: not a subtype of Throwable"
    );
    assert_eq!(errors[0].span, Some(value_span));
    // The user file is the third input (two core files).
    assert_eq!(errors[0].file, 2);
}

/// A non-Unit function may end with `throw` instead of `return`: the
/// throw diverges, so the function never leaves without a value (the
/// M3 return rule, relaxed in M8).
#[test]
fn trailing_throw_satisfies_the_return_rule() {
    let file = file(vec![
        custom_error(),
        fun_sig(
            "fail",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![throw_stmt(call("MyError", vec![int_lit(42)]))],
        ),
        fun("main", vec![stmt(call("println", vec![int_lit(0)]))]),
    ]);
    lower_user_with_exceptions(file).expect("a trailing `throw` must satisfy the return rule");
}

/// A `throw` makes the rest of its sequential block unreachable, so
/// following statements do not make the function fall through.
#[test]
fn non_trailing_throw_satisfies_the_return_rule() {
    let file = file(vec![
        custom_error(),
        fun_sig(
            "f",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![
                throw_stmt(call("MyError", vec![int_lit(1)])),
                val("y", int_lit(1)),
            ],
        ),
        fun("main", vec![stmt(call("println", vec![int_lit(0)]))]),
    ]);
    lower_user_with_exceptions(file).expect("statements after `throw` are unreachable");
}

/// A `try` satisfies the return rule when its try body and every catch
/// body do, while a normally completing `finally` preserves those
/// path results.
#[test]
fn trailing_try_satisfies_the_return_rule() {
    let file = file(vec![
        custom_error(),
        fun_sig(
            "outer",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![try_stmt(
                vec![ret(Some(int_lit(0)))],
                vec![catch_clause(
                    "e",
                    ty_named("MyError"),
                    vec![throw_stmt(var("e"))],
                )],
                Some(vec![stmt(call("println", vec![str_lit("finally")]))]),
            )],
        ),
        fun("main", vec![stmt(call("println", vec![int_lit(0)]))]),
    ]);
    lower_user_with_exceptions(file).expect("a non-falling-through `try` must qualify");
}

/// A trailing `try` whose try body (or a catch body) falls through
/// still trips the M3 return rule.
#[test]
fn falling_through_try_does_not_satisfy_the_return_rule() {
    for (try_body, catch_body) in [
        // The try body falls through.
        (
            vec![stmt(call("println", vec![str_lit("x")]))],
            vec![ret(Some(int_lit(0)))],
        ),
        // A catch body falls through.
        (
            vec![ret(Some(int_lit(0)))],
            vec![stmt(call("println", vec![str_lit("x")]))],
        ),
    ] {
        let file = file(vec![
            custom_error(),
            fun_sig(
                "g",
                vec![],
                vec![],
                Some(ty_named("Int")),
                vec![try_stmt(
                    try_body,
                    vec![catch_clause("e", ty_named("MyError"), catch_body)],
                    Some(vec![stmt(call("println", vec![str_lit("finally")]))]),
                )],
            ),
            fun("main", vec![stmt(call("println", vec![int_lit(0)]))]),
        ]);
        let errors =
            lower_user_with_exceptions(file).expect_err("a falling-through `try` must not qualify");
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(
            errors[0].message,
            "non-Unit function `g` may complete without returning a value"
        );
    }
}

/// A finally that cannot complete normally overrides every pending
/// normal, return, or exceptional path.
#[test]
fn exiting_finally_satisfies_the_return_rule() {
    for finally_body in [
        vec![ret(Some(int_lit(7)))],
        vec![throw_stmt(call("MyError", vec![int_lit(8)]))],
        vec![if_stmt(
            bool_lit(true),
            vec![ret(Some(int_lit(9)))],
            Some(vec![ret(Some(int_lit(10)))]),
        )],
    ] {
        let file = file(vec![
            custom_error(),
            fun_sig(
                "f",
                vec![],
                vec![],
                Some(ty_named("Int")),
                vec![try_stmt(
                    vec![stmt(call("println", vec![str_lit("body")]))],
                    vec![],
                    Some(finally_body),
                )],
            ),
            fun("main", vec![]),
        ]);
        lower_user_with_exceptions(file).expect("an exiting finally overrides every path");
    }
}

#[test]
fn partially_exiting_finally_does_not_hide_fallthrough() {
    let file = file(vec![
        fun_sig(
            "f",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![try_stmt(
                vec![stmt(call("println", vec![str_lit("body")]))],
                vec![],
                Some(vec![if_stmt(
                    bool_lit(true),
                    vec![ret(Some(int_lit(1)))],
                    None,
                )]),
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user_with_exceptions(file)
        .expect_err("a partially exiting finally still has a fallthrough path");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-Unit function `f` may complete without returning a value"
    );
}

// --- negative: catch ---

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
    assert_eq!(errors[0].message, "unknown variable `e`");
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
