//! M12 annotation syntax and lexical safety-block tests.

use scoop_ast::{
    AnnotationLiteral, BinOp, Decl, Expr, FunctionBody, SafetyMode, StatementKind, TypeBound,
    TypeParamKindBound, TypeRefKind, Variance,
};

use crate::tests::{block_body, err, ok, only_function};

#[test]
fn parses_typed_top_level_storage_declarations() {
    let file = ok("@Global var counter: Int = 1\n\
         @Extern(lib = \"native\", name = \"limit\") val limit: Int");
    let Decl::Global(counter) = &file.declarations[0] else {
        panic!("expected global");
    };
    assert!(counter.mutable);
    assert!(counter.init.is_some());
    assert_eq!(counter.annotations[0].name.text, "Global");
    let Decl::Global(limit) = &file.declarations[1] else {
        panic!("expected extern global");
    };
    assert!(!limit.mutable);
    assert!(limit.init.is_none());
    assert_eq!(limit.annotations[0].name.text, "Extern");
}

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

#[test]
fn parses_kind_bounds_on_every_m12_generic_declaration() {
    let file = ok("fun <T : value> identity(value: T): T = value\n\
         struct Box<T : ref>(val value: T)\n\
         enum Choice<T : value> { Some(T), None }\n\
         interface Source<out T : ref> { fun get(): T }");

    let Decl::Function(function) = &file.declarations[0] else {
        panic!("expected function");
    };
    assert_eq!(
        function.type_params[0].inline_bound,
        Some(TypeBound::Kind(TypeParamKindBound::Value))
    );
    let Decl::Struct(decl) = &file.declarations[1] else {
        panic!("expected struct");
    };
    assert_eq!(
        decl.type_params[0].inline_bound,
        Some(TypeBound::Kind(TypeParamKindBound::Ref))
    );
    let Decl::Enum(decl) = &file.declarations[2] else {
        panic!("expected enum");
    };
    assert_eq!(
        decl.type_params[0].inline_bound,
        Some(TypeBound::Kind(TypeParamKindBound::Value))
    );
    let Decl::Interface(decl) = &file.declarations[3] else {
        panic!("expected interface");
    };
    assert_eq!(decl.type_params[0].variance, Variance::Out);
    assert_eq!(
        decl.type_params[0].inline_bound,
        Some(TypeBound::Kind(TypeParamKindBound::Ref))
    );

    let dump = scoop_ast::dump(&file);
    assert!(dump.contains("fun identity<T : value>(value: T): T"));
    assert!(dump.contains("struct Box<T : ref>"));
    assert!(dump.contains("enum Choice<T : value>"));
    assert!(dump.contains("interface Source<out T : ref>"));
}

#[test]
fn parses_interface_upper_bounds_for_hir_validation() {
    let file = ok("fun <T : Comparable> compare(value: T) = value");
    let bound = &only_function(&file).type_params[0].inline_bound;
    assert!(matches!(
        bound,
        Some(TypeBound::Upper(ty))
            if matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "Comparable")
    ));
}

#[test]
fn parses_explicit_type_arguments_on_functions_and_methods() {
    let file = ok("fun main() {\n\
             identity<Int>(1)\n\
             pointer.cast<UInt>()\n\
         }");
    let statements = &block_body(only_function(&file)).statements;
    let StatementKind::Expr(Expr::Call(call)) = &statements[0].kind else {
        panic!("expected function call");
    };
    assert!(matches!(
        &call.type_args[0].kind,
        TypeRefKind::Named(name) if name.text == "Int"
    ));
    let StatementKind::Expr(Expr::MethodCall { type_args, .. }) = &statements[1].kind else {
        panic!("expected method call");
    };
    assert!(matches!(
        &type_args[0].kind,
        TypeRefKind::Named(name) if name.text == "UInt"
    ));
    let dump = scoop_ast::dump(&file);
    assert!(dump.contains("Call identity<Int>"));
    assert!(dump.contains("MethodCall cast<UInt>"));
}

#[test]
fn explicit_type_argument_probe_preserves_comparison_syntax() {
    let file = ok("fun less(left: Int, right: Int): Boolean = left < right");
    let FunctionBody::Expr(body) = &only_function(&file).body else {
        panic!("expected expression body");
    };
    assert!(matches!(body.as_ref(), Expr::Binary { op: BinOp::Lt, .. }));
}
