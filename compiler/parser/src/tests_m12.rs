//! M12 annotation syntax and lexical safety-block tests.

use scoop_ast::{AnnotationLiteral, Decl, FunctionBody, SafetyMode, StatementKind};

use crate::tests::{block_body, err, ok, only_function};

#[test]
fn parses_marker_positional_named_and_multiple_annotations() {
    let file = ok(
        "@NoGC\n@Extern(\"native\", name = \"sum\", enabled = true, version = 12)\nfun sum(): Int = 0",
    );
    let function = only_function(&file);
    assert_eq!(function.annotations.len(), 2);
    assert!(function.annotations[0].args.is_empty());
    let args = &function.annotations[1].args;
    assert_eq!(args.len(), 4);
    assert!(args[0].name.is_none());
    assert!(matches!(&args[0].value, AnnotationLiteral::String(v) if v == "native"));
    assert_eq!(
        args[1].name.as_ref().map(|name| name.text.as_str()),
        Some("name")
    );
    assert!(matches!(&args[2].value, AnnotationLiteral::Boolean(true)));
    assert!(matches!(&args[3].value, AnnotationLiteral::Int(12)));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n    @NoGC\n    @Extern(\"native\", name = \"sum\", enabled = true, version = 12)\n  fun sum(): Int\n    =\n      IntLiteral 0\n"
    );
}

#[test]
fn positional_arguments_must_precede_named_arguments() {
    let (_, message) = err("@Extern(name = \"f\", \"m\") fun f()");
    assert_eq!(
        message,
        "positional annotation arguments must precede named arguments"
    );
}

#[test]
fn annotation_arguments_are_literals_only() {
    let (_, message) = err("@Extern(name = symbol) fun f()");
    assert_eq!(
        message,
        "annotation argument must be a string, integer or boolean literal"
    );
}

#[test]
fn annotations_precede_member_modifiers() {
    let file = ok("struct S(val x: Int) { @NoGC override fun value(): Int = x }");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected struct");
    };
    assert_eq!(decl.methods[0].annotations[0].name.text, "NoGC");
    assert!(decl.methods[0].is_override);
}

#[test]
fn annotated_function_body_is_not_decided_by_the_parser() {
    let with_body = ok("@NoGC fun f(): Int = 1");
    assert!(matches!(
        only_function(&with_body).body,
        FunctionBody::Expr(_)
    ));
    let bodyless = ok("@Extern(name = \"f\") fun f(): Int");
    assert!(matches!(only_function(&bodyless).body, FunctionBody::None));
}

#[test]
fn parses_nested_unsafe_and_safe_blocks_as_dedicated_nodes() {
    let file = ok("fun main() {\n    @Unsafe {\n        @Safe { }\n    }\n}\n");
    let StatementKind::SafetyBlock { mode, block } =
        &block_body(only_function(&file)).statements[0].kind
    else {
        panic!("expected safety block");
    };
    assert_eq!(*mode, SafetyMode::Unsafe);
    let StatementKind::SafetyBlock { mode, .. } = &block.statements[0].kind else {
        panic!("expected nested safety block");
    };
    assert_eq!(*mode, SafetyMode::Safe);
    assert!(scoop_ast::dump(&file).contains("@Unsafe block\n      @Safe block"));
}

#[test]
fn safety_blocks_require_one_marker_annotation() {
    let (_, message) = err("fun main() { @Unsafe(1) {} }");
    assert_eq!(message, "safety block annotations do not accept arguments");

    let (_, message) = err("fun main() { @NoGC {} }");
    assert_eq!(message, "only `@Unsafe` or `@Safe` may annotate a block");
}
