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
fn validates_and_preserves_coroutine_core_contract() {
    let module = lower_user(file(vec![fun("main", vec![])]))
        .expect("the canonical coroutine core protocol should lower");

    let continuation = module
        .interfaces
        .iter()
        .find_map(|(_, interface)| (interface.name == "Continuation").then_some(interface))
        .expect("Continuation interface");
    assert_eq!(continuation.type_params.len(), 1);
    assert_eq!(continuation.methods.len(), 2);
    assert_eq!(
        module.functions[module.interface_methods[continuation.methods[0]].function]
            .name
            .rsplit('.')
            .next(),
        Some("resume")
    );
    assert_eq!(
        module.functions[module.interface_methods[continuation.methods[1]].function]
            .name
            .rsplit('.')
            .next(),
        Some("resumeWithException")
    );
    assert_eq!(
        module.classes[module.exception_core.throwable.class()].name,
        "Throwable"
    );
    assert_eq!(
        module.classes[module.exception_core.illegal_state_exception.class()].name,
        "IllegalStateException"
    );
    assert_eq!(
        module.coroutine_core.continuation_resume,
        function_id(&module, "Continuation.resume")
    );
    assert_eq!(
        module.coroutine_core.continuation_resume_with_exception,
        function_id(&module, "Continuation.resumeWithException")
    );

    let task = module
        .interfaces
        .iter()
        .find_map(|(_, interface)| (interface.name == "SuspendTask").then_some(interface))
        .expect("SuspendTask interface");
    assert_eq!(task.type_params.len(), 1);
    assert!(module.functions[module.interface_methods[task.methods[0]].function].is_suspend);

    let start = function_id(&module, "startCoroutine");
    assert_eq!(module.coroutine_core.start_coroutine, start);
    assert!(matches!(
        &module.functions[start].kind,
        hir::FunctionKind::Intrinsic(intrinsic)
            if intrinsic.kind == hir::IntrinsicFunctionKind::CoroutineStart
    ));
    let suspend = function_id(&module, "suspendCoroutine");
    assert_eq!(module.coroutine_core.suspend_coroutine, suspend);
    assert!(module.functions[suspend].is_suspend);
    assert!(matches!(
        &module.functions[suspend].kind,
        hir::FunctionKind::Intrinsic(intrinsic)
            if intrinsic.kind == hir::IntrinsicFunctionKind::CoroutineSuspend
    ));
}

#[test]
fn rejects_malformed_continuation_core_contract() {
    let mut core = core_file();
    let interface = core
        .declarations
        .iter_mut()
        .find_map(|decl| match decl {
            Decl::Interface(interface) if interface.name.text == "Continuation" => Some(interface),
            _ => None,
        })
        .expect("Continuation declaration");
    interface.methods[0].name.text = "complete".to_string();

    let errors = lower(&[core, file(vec![fun("main", vec![])])])
        .expect_err("the compiler-known interface contract must be exact");
    let contract = errors
        .iter()
        .find(|error| {
            error
                .message
                .starts_with("interface `Continuation<T>` in scoop.core must declare exactly")
        })
        .expect("Continuation contract diagnostic");
    assert_eq!(contract.file, 0);
}

#[test]
fn rejects_malformed_coroutine_start_intrinsic() {
    let mut core = core_file();
    let function = core
        .declarations
        .iter_mut()
        .find_map(|decl| match decl {
            Decl::Function(function) if function.name.text == "startCoroutine" => Some(function),
            _ => None,
        })
        .expect("startCoroutine declaration");
    function.return_ty = Some(ty_named("Int"));

    let errors = lower(&[core, file(vec![fun("main", vec![])])])
        .expect_err("the coroutine_start signature must be exact");
    let contract = errors
        .iter()
        .find(|error| {
            error
                .message
                .starts_with("intrinsic `coroutine_start` must have signature")
        })
        .expect("coroutine_start contract diagnostic");
    assert_eq!(contract.file, 0);
}

#[test]
fn rejects_non_suspend_coroutine_suspend_intrinsic() {
    let mut core = core_file();
    let function = core
        .declarations
        .iter_mut()
        .find_map(|decl| match decl {
            Decl::Function(function) if function.name.text == "suspendCoroutine" => Some(function),
            _ => None,
        })
        .expect("suspendCoroutine declaration");
    function.is_suspend = false;

    let errors = lower(&[core, file(vec![fun("main", vec![])])])
        .expect_err("the coroutine_suspend signature must be suspend");
    let contract = errors
        .iter()
        .find(|error| {
            error
                .message
                .starts_with("intrinsic `coroutine_suspend` must have signature")
        })
        .expect("coroutine_suspend contract diagnostic");
    assert_eq!(contract.file, 0);
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
    let hir::FunctionGenericity::Generic {
        definition: identity_generic,
        ..
    } = module.functions[identity].genericity
    else {
        panic!("identity generic entity")
    };
    let identity_requests: Vec<_> = module
        .instantiations
        .iter()
        .filter(|(_, request)| request.generic == identity_generic)
        .collect();
    assert_eq!(identity_requests.len(), 1);
    assert_eq!(identity_requests[0].1.type_args, [int_type(&module)]);
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
    assert_eq!(
        errors[0].message,
        "missing executable entry: declare exactly one ordinary `fun main(): Unit`"
    );
    assert_eq!(errors[0].notes.len(), 1);
    assert_eq!(
        errors[0].notes[0].message,
        "`main` is not eligible because it is suspend"
    );
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
    assert!(module.functions[module.interface_methods[task.methods[0]].function].is_suspend);
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
