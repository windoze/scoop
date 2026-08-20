use crate::tests::{block_body, ok, only_function};
use scoop_ast::{Expr, StatementKind};

#[test]
fn explicit_and_implicit_lambda_labels_follow_the_call_syntax() {
    for (source, expected) in [
        ("take({ return@take 1 })", "take"),
        ("take chosen@ { return@chosen 1 }", "chosen"),
        ("receiver.take suspend { return@take 1 }", "take"),
        ("receiver.take(chosen@ { return@chosen 1 })", "chosen"),
    ] {
        let file = ok(&format!("fun main() {{ {source} }}"));
        let StatementKind::Expr(call) = &block_body(only_function(&file)).statements[0].kind else {
            panic!("call");
        };
        let args = match call {
            Expr::Call(call) => &call.args,
            Expr::MethodCall { args, .. } => args,
            _ => panic!("named call"),
        };
        let Expr::Lambda { label, body, .. } = &args[0].expression else {
            panic!("lambda");
        };
        assert_eq!(label.as_ref().unwrap().text, expected);
        let StatementKind::Return { label, value } = &body.statements[0].kind else {
            panic!("return");
        };
        assert_eq!(label.as_ref().unwrap().text, expected);
        assert!(value.is_some());
    }
}

#[test]
fn an_invoke_of_a_complex_callee_does_not_invent_a_label() {
    let file = ok("fun main() { (factory()) { 1 } }");
    let StatementKind::Expr(Expr::Invoke { args, .. }) =
        &block_body(only_function(&file)).statements[0].kind
    else {
        panic!("invoke");
    };
    assert!(matches!(
        &args[0].expression,
        Expr::Lambda { label: None, .. }
    ));
}

#[test]
fn anonymous_functions_accept_an_explicit_label() {
    let file = ok("fun main() { val f = exit@ fun(): Int { return@exit 1 } }");
    let StatementKind::ValDecl(value) = &block_body(only_function(&file)).statements[0].kind else {
        panic!("binding");
    };
    assert!(
        matches!(&value.init, Expr::AnonymousFunction { label: Some(label), .. } if label.text == "exit")
    );
}
