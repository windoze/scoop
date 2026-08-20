use crate::tests::ok;
use scoop_ast::{Decl, Expr, FunctionBody, LocalTypeDecl, StatementKind};

#[test]
fn block_nominals_keep_declaration_nodes() {
    let file = ok("fun main() { class C; struct S(); enum E { A }; interface I {} }");
    let Decl::Function(function) = &file.declarations[0] else {
        panic!("function");
    };
    let FunctionBody::Block(body) = &function.body else {
        panic!("block");
    };
    assert!(matches!(
        body.statements[0].kind,
        StatementKind::LocalType(LocalTypeDecl::Class(_))
    ));
    assert!(matches!(
        body.statements[1].kind,
        StatementKind::LocalType(LocalTypeDecl::Struct(_))
    ));
    assert!(matches!(
        body.statements[2].kind,
        StatementKind::LocalType(LocalTypeDecl::Enum(_))
    ));
    assert!(matches!(
        body.statements[3].kind,
        StatementKind::LocalType(LocalTypeDecl::Interface(_))
    ));
}

#[test]
fn annotations_on_local_declarations_are_distinct_from_safety_blocks() {
    let file = ok(
        "fun main() { @Label(1) typealias N = Int; @Label(2) fun f(@Label(3) x: N): N = x; @Unsafe {} }",
    );
    let Decl::Function(function) = &file.declarations[0] else {
        panic!("function")
    };
    let FunctionBody::Block(body) = &function.body else {
        panic!("block")
    };
    let StatementKind::TypeAlias(alias) = &body.statements[0].kind else {
        panic!("alias")
    };
    assert_eq!(alias.annotations.len(), 1);
    let StatementKind::LocalFunction(local) = &body.statements[1].kind else {
        panic!("local function")
    };
    assert_eq!(local.annotations.len(), 1);
    assert_eq!(local.params[0].annotations.len(), 1);
}

#[test]
fn anonymous_objects_are_expressions_with_ordered_members() {
    let file = ok(
        "fun main() { val x = object : Base(1), I { val value: Int = 2; init { f() }; override fun get(): Int = value } }",
    );
    let Decl::Function(function) = &file.declarations[0] else {
        panic!("function");
    };
    let FunctionBody::Block(body) = &function.body else {
        panic!("block");
    };
    let StatementKind::ValDecl(binding) = &body.statements[0].kind else {
        panic!("binding");
    };
    let Expr::AnonymousObject(object) = &binding.init else {
        panic!("object");
    };
    assert_eq!(object.supertypes.len(), 2);
    assert_eq!(object.members.len(), 3);
    ok("fun main() { val object = 42; println(object) }");
    ok("fun main() { @pkg.Marker(values = [1, 2]) class Marked }");
}
