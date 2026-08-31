use scoop_ast::{Decl, FunctionBody, MethodModifier, Span};

use crate::parse;

#[test]
fn parses_suspend_top_level_and_generic_functions() {
    let file = parse("suspend fun <T> await(value: T): T = value\nfun main() {}")
        .expect("suspend declarations should parse");
    let Decl::Function(await_fn) = &file.declarations[0] else {
        panic!("expected a function");
    };
    assert!(await_fn.is_suspend);
    assert_eq!(await_fn.type_params[0].text, "T");
    assert_eq!(await_fn.span, Span::new(0, 42));
    assert!(matches!(await_fn.body, FunctionBody::Expr(_)));
    assert!(scoop_ast::dump(&file).contains("suspend fun await<T>(value: T): T"));
}

#[test]
fn parses_suspend_member_modifiers_in_any_order() {
    let file = parse(
        "abstract class Base { abstract suspend fun load(): Int }\n\
         class Derived : Base() { final suspend override fun load(): Int = 1 }\n\
         interface Task { suspend fun run(): Int }\n\
         fun main() {}",
    )
    .expect("suspend member declarations should parse");

    let Decl::Class(base) = &file.declarations[0] else {
        panic!("expected Base");
    };
    assert!(base.methods[0].is_suspend);
    assert_eq!(base.methods[0].modifier, MethodModifier::Abstract);

    let Decl::Class(derived) = &file.declarations[1] else {
        panic!("expected Derived");
    };
    assert!(derived.methods[0].is_suspend);
    assert!(derived.methods[0].is_override);
    assert_eq!(derived.methods[0].modifier, MethodModifier::Final);

    let Decl::Interface(task) = &file.declarations[2] else {
        panic!("expected Task");
    };
    assert!(task.methods[0].is_suspend);
}

#[test]
fn parses_suspend_intrinsic_declaration() {
    let file = parse("@Intrinsic(\"coroutine_suspend\") suspend fun <T> pause(value: T): T")
        .expect("a suspend intrinsic declaration should parse");
    let Decl::Function(function) = &file.declarations[0] else {
        panic!("expected a function");
    };
    assert!(function.is_suspend);
    assert!(matches!(function.body, FunctionBody::None));
}

#[test]
fn parses_suspend_enum_method_after_modality() {
    let file = parse("enum E { A, final suspend fun value(): Int = 1 }\nfun main() {}")
        .expect("enum member modifiers should accept suspend in any order");
    let Decl::Enum(enum_decl) = &file.declarations[0] else {
        panic!("expected enum E");
    };
    assert!(enum_decl.methods[0].is_suspend);
    assert_eq!(enum_decl.methods[0].modifier, MethodModifier::Final);
}

#[test]
fn suspend_is_reserved_and_duplicate_member_modifier_is_rejected() {
    let reserved = parse("fun suspend() {}").expect_err("suspend cannot be used as an identifier");
    assert_eq!(
        reserved[0].message,
        "expected function name, found `suspend`"
    );

    let duplicate = parse("class C { suspend suspend fun f() {} }\nfun main() {}")
        .expect_err("duplicate suspend must fail");
    assert_eq!(
        duplicate[0].message,
        "duplicate `suspend` modifier on member function"
    );
}

#[test]
fn rejects_duplicate_top_level_suspend_modifier() {
    let diagnostics = parse("suspend suspend fun work() {}\nfun main() {}")
        .expect_err("duplicate top-level suspend must fail");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].span, Some(Span::new(8, 15)));
    assert_eq!(
        diagnostics[0].message,
        "duplicate `suspend` modifier on top-level function"
    );
}

#[test]
fn rejects_suspend_on_non_function_declarations() {
    let diagnostics =
        parse("suspend class Work\nfun main() {}").expect_err("suspend is not a class modifier");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].span, Some(Span::new(0, 7)));
    assert_eq!(
        diagnostics[0].message,
        "`suspend` modifier is only allowed on function declarations"
    );
}

#[test]
fn rejects_suspend_on_non_function_members() {
    let diagnostics = parse("class Owner { final suspend class Nested }\nfun main() {}")
        .expect_err("suspend is not a nested-class modifier");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].span, Some(Span::new(20, 27)));
    assert_eq!(
        diagnostics[0].message,
        "`suspend` modifier is only allowed on function declarations"
    );
}
