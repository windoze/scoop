//! M11 function type syntax.

use scoop_ast::{Decl, Span, TypeRef, TypeRefKind};

use crate::parse;

fn function_type(ty: &TypeRef) -> &scoop_ast::FunctionTypeRef {
    let TypeRefKind::Function(function) = &ty.kind else {
        panic!("expected function type, found {:?}", ty.kind);
    };
    function
}

#[test]
fn parses_ordinary_and_suspend_function_types() {
    let file = parse(
        "fun use(op: (Int, String) -> Boolean, task: suspend () -> Int): (Int) -> String = op\n\
         fun main() {}",
    )
    .expect("function types should parse");
    let Decl::Function(use_) = &file.declarations[0] else {
        panic!("expected use");
    };

    let op = function_type(&use_.params[0].ty);
    assert!(!op.is_suspend);
    assert_eq!(op.parameters.len(), 2);
    assert!(matches!(op.parameters[0].kind, TypeRefKind::Named(_)));
    assert!(matches!(op.return_type.kind, TypeRefKind::Named(_)));

    let task = function_type(&use_.params[1].ty);
    assert!(task.is_suspend);
    assert!(task.parameters.is_empty());

    let result = function_type(use_.return_ty.as_ref().expect("return type"));
    assert_eq!(result.parameters.len(), 1);
    assert_eq!(
        scoop_ast::dump(&file),
        "SourceFile\n  fun use(op: (Int, String) -> Boolean, task: suspend () -> Int): (Int) -> String\n    =\n      Var op\n  fun main()\n"
    );
}

#[test]
fn function_types_nest_and_compose_with_nullable_and_generic_types() {
    let file = parse(
        "fun nested(value: Option<((Int) -> String)?>): () -> (Int) -> String = value\n\
         fun main() {}",
    )
    .expect("nested function types should parse");
    let Decl::Function(nested) = &file.declarations[0] else {
        panic!("expected nested");
    };
    let TypeRefKind::Generic(_, args) = &nested.params[0].ty.kind else {
        panic!("expected Option application");
    };
    assert!(matches!(args[0].kind, TypeRefKind::Nullable(_)));
    let outer = function_type(nested.return_ty.as_ref().expect("return type"));
    assert!(outer.parameters.is_empty());
    assert!(matches!(outer.return_type.kind, TypeRefKind::Function(_)));
}

#[test]
fn tuple_parentheses_and_function_parameters_are_disambiguated() {
    let file = parse(
        "fun forms(tuple: (Int, String), paren: (Int), one: (Int,) -> String): Unit {}\n\
         fun main() {}",
    )
    .expect("type forms should disambiguate");
    let Decl::Function(forms) = &file.declarations[0] else {
        panic!("expected forms");
    };
    assert!(matches!(forms.params[0].ty.kind, TypeRefKind::Tuple(_)));
    assert!(matches!(forms.params[1].ty.kind, TypeRefKind::Named(_)));
    assert_eq!(function_type(&forms.params[2].ty).parameters.len(), 1);
}

#[test]
fn suspend_requires_a_function_arrow() {
    let diagnostics = parse("fun use(task: suspend (Int)) {}\nfun main() {}")
        .expect_err("suspend tuple must not parse as a type");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].span, Some(Span::new(27, 28)));
    assert_eq!(
        diagnostics[0].message,
        "expected `->` in suspend function type, found `)`"
    );
}

#[test]
fn function_type_requires_a_return_type() {
    let diagnostics = parse("fun use(op: (Int) ->) {}\nfun main() {}")
        .expect_err("missing return type must fail");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].span, Some(Span::new(20, 21)));
    assert_eq!(diagnostics[0].message, "expected type, found `)`");
}
