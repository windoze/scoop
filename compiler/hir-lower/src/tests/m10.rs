use scoop_hir as hir;

use super::*;

fn function_id(module: &hir::Module, name: &str) -> hir::FunctionId {
    module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing function {name}"))
}

#[test]
fn suspend_flag_reaches_hir_and_suspend_body_may_call_suspend() {
    let module = lower_user(file(vec![
        suspend_fun("leaf", vec![]),
        suspend_fun("caller", vec![stmt(call("leaf", vec![]))]),
        fun("main", vec![]),
    ]))
    .expect("suspend-to-suspend calls should lower");

    let leaf = function_id(&module, "leaf");
    let caller = function_id(&module, "caller");
    assert!(module.functions[leaf].is_suspend);
    assert!(module.functions[caller].is_suspend);
    assert!(hir::dump(&module).contains("suspend fun leaf(): Unit"));
}

#[test]
fn generic_suspend_call_keeps_typed_callee_identity() {
    let Decl::Function(mut identity) = fun_sig(
        "identity",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        vec![ret(Some(var("value")))],
    ) else {
        unreachable!()
    };
    identity.is_suspend = true;
    let module = lower_user(file(vec![
        Decl::Function(identity),
        suspend_fun("caller", vec![stmt(call("identity", vec![int_lit(1)]))]),
        fun("main", vec![]),
    ]))
    .expect("generic suspend calls should lower");
    let identity = function_id(&module, "identity");
    assert!(module.functions[identity].is_suspend);
    assert_eq!(module.generic_functions.len(), 1);
    assert_eq!(module.instantiations.len(), 1);
}

#[test]
fn non_suspend_function_cannot_call_suspend_function() {
    let errors = lower_user(file(vec![
        suspend_fun("wait", vec![]),
        fun("main", vec![stmt(call("wait", vec![]))]),
    ]))
    .expect_err("ordinary callers must be rejected");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "suspend function `wait` cannot be called from non-suspend function `main`"
    );
}

#[test]
fn non_suspend_method_cannot_call_suspend_method() {
    let errors = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Worker",
            vec![],
            None,
            vec![],
            vec![
                with_suspend(method("wait", vec![], None, vec![])),
                method("run", vec![], None, vec![stmt(call("wait", vec![]))]),
            ],
        ),
        fun("main", vec![]),
    ]))
    .expect_err("ordinary methods cannot call suspend methods");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "suspend function `Worker.wait` cannot be called from non-suspend function `run`"
    );
}

#[test]
fn main_cannot_be_suspend() {
    let errors = lower_user(file(vec![suspend_fun("main", vec![])]))
        .expect_err("the process entry point cannot suspend");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`main` must not be suspend");
}

#[test]
fn suspend_is_not_an_overload_discriminator() {
    let errors = lower_user(file(vec![
        fun("same", vec![]),
        suspend_fun("same", vec![]),
        fun("main", vec![]),
    ]))
    .expect_err("ordinary and suspend forms with equal parameters are duplicates");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "function `same` is already declared with the same signature"
    );
}

#[test]
fn override_must_match_suspend_contract() {
    let interface_method =
        with_suspend(bodyless_method(false, "run", vec![], Some(ty_named("Int"))));
    let implementation = override_method_expr("run", vec![], Some(ty_named("Int")), int_lit(1));
    let errors = lower_user(file(vec![
        interface_decl("Task", vec![interface_method]),
        class_decl(
            ast::ClassModifier::Final,
            "TaskImpl",
            vec![],
            None,
            vec!["Task"],
            vec![implementation],
        ),
        fun("main", vec![]),
    ]))
    .expect_err("an implementation cannot drop suspend");
    assert_eq!(errors.len(), 2);
    assert_eq!(
        errors[0].message,
        "`run` must have the same `suspend` modifier as `Task.run`"
    );
    assert_eq!(
        errors[1].message,
        "class `TaskImpl` does not implement interface method `Task.run`"
    );
}

#[test]
fn matching_suspend_interface_implementation_is_preserved() {
    let interface_method =
        with_suspend(bodyless_method(false, "run", vec![], Some(ty_named("Int"))));
    let implementation = with_suspend(override_method_expr(
        "run",
        vec![],
        Some(ty_named("Int")),
        int_lit(1),
    ));
    let module = lower_user(file(vec![
        interface_decl("Task", vec![interface_method]),
        class_decl(
            ast::ClassModifier::Final,
            "TaskImpl",
            vec![],
            None,
            vec!["Task"],
            vec![implementation],
        ),
        fun("main", vec![]),
    ]))
    .expect("matching suspend contracts should lower");
    let task = module
        .interfaces
        .iter()
        .find_map(|(_, interface)| (interface.name == "Task").then_some(interface))
        .expect("Task interface");
    assert!(task.methods[0].is_suspend);
    assert!(module.functions[function_id(&module, "TaskImpl.run")].is_suspend);
}

#[test]
fn constructor_delegation_cannot_call_suspend() {
    let errors = lower_user(file(vec![
        Decl::Function({
            let Decl::Function(mut wait) = fun_sig(
                "wait",
                vec![],
                vec![],
                Some(ty_named("Int")),
                vec![ret(Some(int_lit(1)))],
            ) else {
                unreachable!()
            };
            wait.is_suspend = true;
            wait
        }),
        class_decl(
            ast::ClassModifier::Open,
            "Base",
            vec![(false, "value", ty_named("Int"))],
            None,
            vec![],
            vec![],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Derived",
            vec![],
            Some(("Base", vec![call("wait", vec![])])),
            vec![],
            vec![],
        ),
        fun("main", vec![]),
    ]))
    .expect_err("constructor delegation is a non-suspend initialization context");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "suspend function `wait` cannot be called from constructor delegation"
    );
}
