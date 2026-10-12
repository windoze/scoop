use crate::tests::{block_body, ok, only_function};
use scoop_ast::{Expr, StatementKind, TypeRefKind};

#[test]
fn receiver_types_keep_their_slot_and_suffixes() {
    for receiver in ["Int", "Int?", "Int??", "(Int, String)", "((Int) -> Int)"] {
        for effect in ["", "suspend "] {
            let file = ok(&format!(
                "fun take(f: {effect}{receiver}.(String) -> Int) {{}}"
            ));
            let TypeRefKind::Function(signature) = &only_function(&file).params[0].ty.kind else {
                panic!("function type");
            };
            assert_eq!(signature.is_suspend, !effect.is_empty());
            assert!(signature.receiver.is_some());
            assert_eq!(signature.parameters.len(), 1);
            if receiver == "Int?" {
                assert!(matches!(
                    signature.receiver.as_ref().unwrap().kind,
                    TypeRefKind::Nullable(_)
                ));
            }
        }
    }
}

#[test]
fn receiver_anonymous_functions_are_expressions_in_statement_position() {
    for source in [
        "fun Int.(): Int { return this }",
        "suspend fun Int?.(): Int { return 1 }",
        "fun (Int, String).(): Int { return this._0 }",
        "fun ((Int) -> Int).(): Int { return this(1) }",
    ] {
        let file = ok(&format!("fun main() {{ {source} }}"));
        let StatementKind::Expr(Expr::AnonymousFunction { receiver, .. }) =
            &block_body(only_function(&file)).statements[0].kind
        else {
            panic!("anonymous receiver function");
        };
        assert!(receiver.is_some());
    }
    let file = ok("fun main() { fun named(value: Int) = value }");
    assert!(matches!(
        block_body(only_function(&file)).statements[0].kind,
        StatementKind::LocalFunction(_)
    ));
}

#[test]
fn labelled_this_remains_an_invocable_expression() {
    let file = ok("fun main() { this@chosen(1) }");
    let StatementKind::Expr(Expr::Invoke { callee, .. }) =
        &block_body(only_function(&file)).statements[0].kind
    else {
        panic!("invoke");
    };
    assert!(
        matches!(callee.as_ref(), Expr::This { label: Some(label), .. } if label.text == "chosen")
    );
}
