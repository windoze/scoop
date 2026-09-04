use scoop_ast::{CallTypeArgument, Decl, Expr, StatementKind, TypeRefKind};

use crate::tests::{block_body, err, ok, only_function};

#[test]
fn call_type_arguments_distinguish_fixed_and_inferred_positions() {
    let file = ok("fun main() {\n\
             pair<Int, _>(1, \"value\")\n\
             receiver.convert<_, String>(1)\n\
             (receiver)<Boolean, _>(1)\n\
         }");
    let statements = &block_body(only_function(&file)).statements;
    let StatementKind::Expr(Expr::Call(call)) = &statements[0].kind else {
        panic!("expected a named call")
    };
    assert!(matches!(
        &call.type_args[..],
        [CallTypeArgument::Explicit(ty), CallTypeArgument::Infer { .. }]
            if matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "Int")
    ));
    let StatementKind::Expr(Expr::MethodCall { type_args, .. }) = &statements[1].kind else {
        panic!("expected a method call")
    };
    assert!(matches!(
        &type_args[..],
        [CallTypeArgument::Infer { .. }, CallTypeArgument::Explicit(ty)]
            if matches!(&ty.kind, TypeRefKind::Named(name) if name.text == "String")
    ));
    let StatementKind::Expr(Expr::Invoke { type_args, .. }) = &statements[2].kind else {
        panic!("expected a function-value invocation")
    };
    assert!(matches!(
        &type_args[..],
        [
            CallTypeArgument::Explicit(_),
            CallTypeArgument::Infer { .. }
        ]
    ));
    let dump = scoop_ast::dump(&file);
    assert!(dump.contains("Call pair<Int, _>"));
    assert!(dump.contains("MethodCall convert<_, String>"));
    assert!(dump.contains("Invoke<Boolean, _>"));
}

#[test]
fn underscore_is_rejected_in_ordinary_type_positions() {
    let (_, message) = err("fun invalid(value: _): Unit {}\n");
    assert_eq!(message, "`_` is only allowed in call type argument lists");

    let (_, message) = err("fun <T : _> invalid(value: T): T = value\n");
    assert_eq!(message, "`_` is only allowed in call type argument lists");

    let (_, message) = err("struct Pair<A, B>(val first: A, val second: B)\n\
         fun <T> identity(value: T): T = value\n\
         fun main() { identity<Pair<Int, _>>(Pair(1, \"value\")) }\n");
    assert_eq!(message, "`_` is only allowed in call type argument lists");
}

#[test]
fn inferred_type_argument_requires_an_immediate_call() {
    let (_, message) = err("fun main() { value<_> }\n");
    assert_eq!(
        message,
        "`_` is only allowed in a type argument list immediately followed by a call"
    );
}

#[test]
fn super_method_call_preserves_inferred_type_arguments() {
    let file = ok(
        "open class Base { fun <T, U> choose(first: T, second: U): U = second }\n\
         class Child : Base() { fun read(): String = super.choose<_, String>(1, \"ok\") }\n",
    );
    let Decl::Class(child) = &file.declarations[1] else {
        panic!("expected child class")
    };
    let scoop_ast::ClassMember::Function(read) = &child.members[0] else {
        panic!("expected read method")
    };
    let scoop_ast::FunctionBody::Expr(body) = &read.body else {
        panic!("expected expression body")
    };
    let Expr::SuperMethodCall { type_args, .. } = body.as_ref() else {
        panic!("expected a super method call")
    };
    assert!(matches!(
        &type_args[..],
        [
            CallTypeArgument::Infer { .. },
            CallTypeArgument::Explicit(_)
        ]
    ));
}
