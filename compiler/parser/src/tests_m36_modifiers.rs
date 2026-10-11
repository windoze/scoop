//! Modifier presence and diagnostics use the same source-bearing value.

use scoop_ast::{Decl, MethodModifier, Span};

use crate::tests::{err, ok};

#[test]
fn modifier_combinations_preserve_function_semantics() {
    let file = ok(
        "abstract class C {\n    public abstract suspend fun load(): Int\n    final override fun value(): Int = 7\n}\n",
    );
    let Decl::Class(class) = &file.declarations[0] else {
        panic!("expected class");
    };
    let functions: Vec<_> = class.functions().collect();
    assert!(functions[0].is_suspend);
    assert_eq!(functions[0].modifier, MethodModifier::Abstract);
    assert!(functions[1].is_override);
    assert_eq!(functions[1].modifier, MethodModifier::Final);
}

#[test]
fn repeated_modifiers_report_the_second_keyword() {
    for (source, keyword, message) in [
        (
            "suspend suspend fun f() {}",
            "suspend",
            "duplicate `suspend` modifier on top-level function",
        ),
        (
            "class C { suspend suspend fun f() {} }",
            "suspend",
            "duplicate `suspend` modifier on member function",
        ),
        (
            "const const val x: Int = 1",
            "const",
            "duplicate `const` modifier on property",
        ),
        (
            "class C { override override fun f() {} }",
            "override",
            "duplicate `override` modifier on member function",
        ),
        (
            "class C { open open fun f() {} }",
            "open",
            "duplicate `open` modifier on member function",
        ),
    ] {
        let (span, actual) = err(source);
        let start = source.rfind(keyword).unwrap() as u32;
        assert_eq!(span, Span::new(start, start + keyword.len() as u32));
        assert_eq!(actual, message);
    }
}

#[test]
fn invalid_modifier_targets_report_the_complete_keyword() {
    for (source, keyword, message) in [
        (
            "public override val x: Int = 1",
            "override",
            "member modality and `override` are not allowed on top-level properties",
        ),
        (
            "public const fun f() {}",
            "const",
            "`const` is only allowed on `val` properties",
        ),
        (
            "public suspend val x: Int = 1",
            "suspend",
            "properties cannot be `suspend`, `operator`, or `infix`",
        ),
        (
            "interface I { public final fun f() {} }",
            "final",
            "`final` modifier is not allowed on interface methods",
        ),
        (
            "interface I { public open fun f() {} }",
            "open",
            "`open` modifier is not allowed on interface methods",
        ),
        (
            "public override class C {}",
            "override",
            "invalid modifier on class declaration",
        ),
    ] {
        let (span, actual) = err(source);
        let start = source.find(keyword).unwrap() as u32;
        assert_eq!(span, Span::new(start, start + keyword.len() as u32));
        assert_eq!(actual, message);
    }
}
