//! M11 function type syntax.

use scoop_ast::{Decl, Expr, Span, StatementKind, TypeRef, TypeRefKind};

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

#[test]
fn parses_lambdas_references_and_function_value_invocation() {
    let file = parse(
        "fun inc(value: Int): Int = value + 1\n\
         fun main() {\n\
           val inferred: (Int) -> Int = { value -> value + 1 }\n\
           val explicit = { value: Int -> value + 2 }\n\
           val zero = { -> 42 }\n\
           val reference: (Int) -> Int = ::inc\n\
           reference.invoke(1)\n\
           ({ value: Int -> value })(2)\n\
         }",
    )
    .expect("M11 function-value syntax should parse");
    let Decl::Function(main) = &file.declarations[1] else {
        panic!("expected main");
    };
    let scoop_ast::FunctionBody::Block(body) = &main.body else {
        panic!("expected block body");
    };
    let StatementKind::ValDecl(first) = &body.statements[0].kind else {
        panic!("expected first val");
    };
    let Expr::Lambda { parameters, .. } = &first.init else {
        panic!("expected lambda");
    };
    assert_eq!(parameters.as_ref().expect("explicit header").len(), 1);
    assert!(parameters.as_ref().unwrap()[0].ty.is_none());
    let StatementKind::ValDecl(zero) = &body.statements[2].kind else {
        panic!("expected zero val");
    };
    assert!(matches!(
        zero.init,
        Expr::Lambda {
            parameters: Some(ref parameters),
            ..
        } if parameters.is_empty()
    ));
    let StatementKind::ValDecl(reference) = &body.statements[3].kind else {
        panic!("expected reference val");
    };
    assert!(matches!(
        reference.init,
        Expr::CallableReference { receiver: None, .. }
    ));
    assert!(matches!(
        body.statements[5].kind,
        StatementKind::Expr(Expr::Invoke { .. })
    ));
}

#[test]
fn omitted_lambda_parameter_list_remains_distinct() {
    let file = parse("fun main() { val operation: (Int) -> Int = { it + 1 } }")
        .expect("omitted parameter header should parse");
    let Decl::Function(main) = &file.declarations[0] else {
        panic!("expected main");
    };
    let scoop_ast::FunctionBody::Block(body) = &main.body else {
        panic!("expected block");
    };
    let StatementKind::ValDecl(decl) = &body.statements[0].kind else {
        panic!("expected val");
    };
    assert!(matches!(
        decl.init,
        Expr::Lambda {
            parameters: None,
            ..
        }
    ));
}

#[test]
fn parses_anonymous_function_with_local_return() {
    let file = parse("fun main() { val operation = fun(value: Int): Int { return value + 1 } }")
        .expect("anonymous function should parse");
    let Decl::Function(main) = &file.declarations[0] else {
        panic!("expected main");
    };
    let scoop_ast::FunctionBody::Block(body) = &main.body else {
        panic!("expected block");
    };
    let StatementKind::ValDecl(decl) = &body.statements[0].kind else {
        panic!("expected val");
    };
    let Expr::AnonymousFunction {
        params,
        return_ty,
        body,
        ..
    } = &decl.init
    else {
        panic!("expected anonymous function");
    };
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].name.text, "value");
    assert!(return_ty.is_some());
    assert!(matches!(
        body.statements[0].kind,
        StatementKind::Return { .. }
    ));
}

#[test]
fn parses_local_functions_as_block_declarations() {
    let file = parse(
        "fun main() {\n  fun <T> identity(value: T): T = value\n  suspend fun task(): Int { return 1 }\n  identity(42)\n}",
    )
    .expect("local functions should parse as declarations");
    let Decl::Function(main) = &file.declarations[0] else {
        panic!("expected main");
    };
    let scoop_ast::FunctionBody::Block(body) = &main.body else {
        panic!("expected block");
    };
    let StatementKind::LocalFunction(identity) = &body.statements[0].kind else {
        panic!("expected local identity declaration");
    };
    assert_eq!(identity.name.text, "identity");
    assert_eq!(identity.type_params.len(), 1);
    let StatementKind::LocalFunction(task) = &body.statements[1].kind else {
        panic!("expected local task declaration");
    };
    assert!(task.is_suspend);
}

#[test]
fn callable_reference_requires_a_name() {
    let diagnostics = parse("fun main() { val operation = :: }")
        .expect_err("a bare double colon must be rejected");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "expected callable name after `::`, found `}`"
    );
}
