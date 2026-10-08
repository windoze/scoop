use super::*;
use crate::tests::{call, fun_expr};

#[test]
fn compiler_exception_adapter_does_not_shadow_the_source_constructor() {
    let mut core = core_file();
    core.declarations.push(fun_expr(
        "constructException",
        vec!["T"],
        vec![],
        Some(ty_named("IllegalArgumentException")),
        call("IllegalArgumentException", vec![]),
    ));
    let output = lower_fixture_with_core(false, core);
    let module = &output.export;
    let function = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "constructException")
        .unwrap()
        .1;
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("the generic source function has a body")
    };
    let hir::StatementKind::Return { value: Some(value) } = &body.statements.last().unwrap().kind
    else {
        panic!("the function returns its constructed exception")
    };
    let hir::ExprKind::ClassInit { constructor, args } = &value.kind else {
        panic!("the expression uses the source constructor")
    };
    let hir::ClassConstructorDefinition::Local(constructor) =
        module.class_constructor_applications[*constructor].constructor
    else {
        panic!("the constructor is defined by this core")
    };
    assert_eq!(
        module.class_constructors[constructor].identity_kind,
        hir::ClassConstructorIdentityKind::Source
    );
    assert_eq!(args.len(), 1, "the source default argument is materialized");
}
