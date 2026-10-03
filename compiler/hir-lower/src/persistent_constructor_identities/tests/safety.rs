use super::*;

#[test]
fn generated_exception_adapters_cannot_change_source_safety() {
    let output = lower_fixture(false);
    let (adapter, _) = output
        .export
        .class_constructors
        .iter()
        .find(|(_, constructor)| {
            matches!(
                constructor.identity_kind,
                hir::ClassConstructorIdentityKind::ZeroArgumentAdapter { .. }
            )
        })
        .unwrap();
    let mut module = output.export.module().clone();
    assert!(rebuild(&module).is_ok());
    module.class_constructors[adapter].safety = hir::Safety::Unsafe;
    assert!(matches!(
        rebuild(&module),
        Err(hir::HirConstructorIdentityError::AdapterShape { .. })
    ));
}
