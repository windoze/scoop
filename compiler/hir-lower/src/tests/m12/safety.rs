use super::*;

#[test]
fn unsafe_calls_follow_the_lexical_safety_stack() {
    let dangerous = annotate(fun("dangerous", vec![]), vec![marker("Unsafe")]);
    let errors = messages(vec![
        dangerous.clone(),
        fun("main", vec![stmt(call("dangerous", vec![]))]),
    ]);
    assert!(errors[0].contains("may only be called from an unsafe context"));

    lower_user(file(vec![
        dangerous.clone(),
        fun(
            "main",
            vec![safety_block(
                ast::SafetyMode::Unsafe,
                vec![stmt(call("dangerous", vec![]))],
            )],
        ),
    ]))
    .expect("unsafe block authorizes the call");

    let errors = messages(vec![
        dangerous,
        fun(
            "main",
            vec![safety_block(
                ast::SafetyMode::Unsafe,
                vec![safety_block(
                    ast::SafetyMode::Safe,
                    vec![stmt(call("dangerous", vec![]))],
                )],
            )],
        ),
    ]);
    assert!(errors[0].contains("may only be called from an unsafe context"));
}

#[test]
fn unsafe_function_body_starts_in_unsafe_context() {
    let leaf = annotate(fun("leaf", vec![]), vec![marker("Unsafe")]);
    let caller = annotate(
        fun("caller", vec![stmt(call("leaf", vec![]))]),
        vec![marker("Unsafe")],
    );
    lower_user(file(vec![leaf, caller, fun("main", vec![])]))
        .expect("unsafe body may call unsafe function");
}

#[test]
fn managed_callable_reference_cannot_erase_unsafe_effect() {
    let leaf = annotate(fun("leaf", vec![]), vec![marker("Unsafe")]);
    let reference = Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("leaf"),
        span: sp(),
    };
    let errors = messages(vec![leaf, fun("main", vec![val("f", reference)])]);
    assert!(errors[0].contains("safety is not part of function-type identity"));
}
