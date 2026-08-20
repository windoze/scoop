use super::*;

#[test]
fn no_gc_unsafe_functions_can_use_stack_addresses_and_pointer_intrinsics() {
    let function = annotate(
        fun_sig(
            "rewrite",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![
                val("pointer", call("addressOf", vec![var("value")])),
                stmt(method_call(var("pointer"), "store", vec![int_lit(12)])),
                ret(Some(method_call(var("pointer"), "load", vec![]))),
            ],
        ),
        vec![marker("NoGC"), marker("Unsafe")],
    );
    lower_user(file(vec![function, fun("main", vec![])]))
        .expect("stack addressing and raw pointer operations remain GC-free");
}

#[test]
fn no_gc_rejects_managed_signatures_calls_and_implicit_exception_ops() {
    let string_param = annotate(
        fun_sig(
            "stringParam",
            vec![],
            vec![("value", ty_named("String"))],
            None,
            vec![],
        ),
        vec![marker("NoGC")],
    );
    let managed = fun("managed", vec![]);
    let calls_managed = annotate(
        fun("callsManaged", vec![stmt(call("managed", vec![]))]),
        vec![marker("NoGC")],
    );
    let divides = annotate(
        fun_expr(
            "divides",
            vec![],
            vec![],
            Some(ty_named("Int")),
            Expr::Binary {
                op: ast::BinOp::Div,
                lhs: Box::new(int_lit(4)),
                rhs: Box::new(int_lit(2)),
                span: sp(),
            },
        ),
        vec![marker("NoGC")],
    );
    let errors = messages(vec![
        string_param,
        managed,
        calls_managed,
        divides,
        fun("main", vec![]),
    ]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("non-GC-free parameter `value`"))
    );
    assert!(
        errors
            .iter()
            .any(|message| message.contains("may not call managed function `managed`"))
    );
    assert!(
        errors
            .iter()
            .any(|message| message.contains("integer division is not allowed"))
    );
}

#[test]
fn no_gc_rejects_reference_receiver_methods() {
    let method = annotate_method(method("work", vec![], None, vec![]), vec![marker("NoGC")]);
    let class = class_decl(
        ast::ClassModifier::Final,
        "Worker",
        vec![],
        None,
        vec![],
        vec![method],
    );
    let errors = messages(vec![class, fun("main", vec![])]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("parameter `this` of type Worker"))
    );
}
