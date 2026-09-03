use super::*;

// --- annotations --------------------------------------------------------------

#[test]
fn intrinsic_annotation_on_bodiless_function() {
    let file = ok("@Intrinsic(\"rt_print\")\nfun print(message: String)\n");
    let function = only_function(&file);
    assert_eq!(function.annotations.len(), 1);
    let annotation = &function.annotations[0];
    assert_eq!(annotation.name.text, "Intrinsic");
    assert!(matches!(
        annotation.args.as_slice(),
        [scoop_ast::AnnotationArg {
            value: scoop_ast::AnnotationLiteral::String(value),
            ..
        }] if value == "rt_print"
    ));
    assert_eq!(annotation.span, Span::new(0, 22));
    assert_eq!(function.name.text, "print");
    assert_eq!(function.span, Span::new(0, 49));
    // A bodiless `@Intrinsic` function (spec 13.1): `FunctionBody::None`
    // since M6; `annotations` is what HIR keys on.
    assert!(matches!(function.body, FunctionBody::None));
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n    @Intrinsic(\"rt_print\")\n  fun print(message: String)\n"
    );
}

#[test]
fn intrinsic_function_followed_by_another_declaration() {
    let file = ok("@Intrinsic(\"rt_print\")\nfun print(message: String)\n\nfun main() {}\n");
    assert_eq!(file.declarations.len(), 2);
    assert!(matches!(file.declarations[0], Decl::Function(_)));
    assert!(matches!(file.declarations[1], Decl::Function(_)));
}

#[test]
fn intrinsic_function_with_return_type() {
    let file = ok("@Intrinsic(\"int_add\")\nfun add(lhs: Int, rhs: Int): Int\n");
    let function = only_function(&file);
    assert!(function.return_ty.is_some());
    assert_eq!(function.params.len(), 2);
}

#[test]
fn unknown_annotation_is_preserved_for_hir() {
    let file = ok("@Unknown fun f() {}");
    assert_eq!(only_function(&file).annotations[0].name.text, "Unknown");
}

#[test]
fn marker_annotation_has_no_arguments() {
    let file = ok("@Intrinsic fun f() {}");
    assert!(only_function(&file).annotations[0].args.is_empty());
}

#[test]
fn empty_annotation_argument_list_is_preserved() {
    let file = ok("@Intrinsic() fun f() {}");
    assert!(only_function(&file).annotations[0].args.is_empty());
}

#[test]
fn annotation_integer_argument_is_preserved() {
    let file = ok("@Intrinsic(42) fun f() {}");
    assert!(matches!(
        only_function(&file).annotations[0].args[0].value,
        scoop_ast::AnnotationLiteral::Int(42)
    ));
}

#[test]
fn multiple_annotation_arguments_are_preserved() {
    let file = ok("@Intrinsic(\"a\", \"b\") fun f() {}");
    assert_eq!(only_function(&file).annotations[0].args.len(), 2);
}

#[test]
fn multiple_annotations_are_preserved() {
    let file = ok("@Intrinsic(\"a\")\n@Intrinsic(\"b\")\nfun f() {}\n");
    assert_eq!(only_function(&file).annotations.len(), 2);
}

#[test]
fn annotation_does_not_suppress_a_block_body() {
    let file = ok("@Intrinsic(\"x\") fun f() {}");
    assert!(matches!(only_function(&file).body, FunctionBody::Block(_)));
}

#[test]
fn annotation_does_not_suppress_an_expression_body() {
    let file = ok("@Intrinsic(\"x\") fun f() = 1");
    assert!(matches!(only_function(&file).body, FunctionBody::Expr(_)));
}

#[test]
fn annotation_on_a_struct_is_preserved_for_hir() {
    let file = ok("@Intrinsic(\"x\") struct S(val x: Int)");
    let Decl::Struct(decl) = &file.declarations[0] else {
        panic!("expected struct");
    };
    assert_eq!(decl.annotations[0].name.text, "Intrinsic");
}
