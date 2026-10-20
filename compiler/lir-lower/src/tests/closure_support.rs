use super::*;

mod fixture;
use fixture::sample;

fn closure_descriptors(module: &lir::Module) -> usize {
    descriptor_values(module)
        .filter(|descriptor| descriptor.diagnostic_name == "testClosure")
        .count()
}

fn signature_descriptors(module: &lir::Module) -> usize {
    descriptor_values(module)
        .filter(|descriptor| {
            matches!(
                descriptor.relations,
                lir::TypeDescriptorRelations::Signature { .. }
            )
        })
        .count()
}

#[test]
fn unreferenced_closure_support_is_omitted_without_changing_semantic_records() {
    let sample = sample();
    let input = seal_strong_input(sample.module);
    let output = lower_test_input(&input).unwrap();
    let module = output.module();
    assert_eq!(input.module().closure_classes.len(), 1);
    assert_eq!(input.module().functions.len(), 4);
    assert_eq!(module.functions.len(), 3); // caller, main, root gateway
    assert_eq!(closure_descriptors(module), 0);
    assert_eq!(signature_descriptors(module), 0);
    assert!(!layout_values(module).any(|layout| layout.name.starts_with("testClosure")));
}

#[test]
fn one_remaining_instance_retains_its_shared_support() {
    let mut sample = sample();
    sample.allocate();
    let module = lower(sample.module);
    assert_eq!(module.functions.len(), 5);
    assert_eq!(closure_descriptors(&module), 1);
    assert_eq!(signature_descriptors(&module), 1);
    assert_eq!(
        layout_values(&module)
            .filter(|layout| layout.name.starts_with("testClosure"))
            .count(),
        2
    );
}

#[test]
fn direct_private_invoke_or_bridge_reference_retains_support() {
    for bridge in [false, true] {
        let mut sample = sample();
        sample.call(
            sample.caller,
            if bridge { sample.bridge } else { sample.invoke },
        );
        let module = lower(sample.module);
        assert_eq!(module.functions.len(), 5);
        assert_eq!(closure_descriptors(&module), 1);
    }
}

#[test]
fn type_test_retains_signature_without_dead_environment() {
    let mut sample = sample();
    let function = &mut sample.module.functions[sample.caller];
    let checked = expr(
        mir::Type::Boolean,
        mir::ExprKind::IsInstance {
            operand: Box::new(local_expr(
                function.params[0].local,
                function.params[0].ty.clone(),
            )),
            check_ty: Box::new(mir::Type::Function(sample.function_type)),
        },
    );
    function.body.blocks[function.body.entry]
        .statements
        .push(stmt(mir::StatementKind::Expr(checked)));
    let module = lower(sample.module);
    assert_eq!(closure_descriptors(&module), 0);
    assert_eq!(signature_descriptors(&module), 1);
}

#[test]
fn dispatch_reference_keeps_private_helpers() {
    let mut sample = sample();
    let (_, class) = sample.module.classes.iter_mut().next().unwrap();
    class.vtable.push(mir::TableSlot::Function(sample.bridge));
    let selected = crate::closure_support::ClosureSupport::new(&sample.module);
    assert!(selected.emits_function(sample.invoke));
    assert!(selected.emits_closure(sample.class));
}

#[test]
fn suspend_closures_are_not_pruned_by_ordinary_support_selection() {
    let mut sample = sample();
    sample.module.function_types[sample.function_type].is_suspend = true;
    let selected = crate::closure_support::ClosureSupport::new(&sample.module);
    assert!(selected.emits_function(sample.invoke));
    assert!(selected.emits_closure(sample.class));
}
