use super::*;

#[test]
fn throw_consumes_the_prepared_reference_and_actual_byte_array_type() {
    let mut h = Harness::new();
    let exception = h.exception("Failure");
    let exception_ty = h.class_ty(exception);
    let mut locals = Arena::new();
    let input = locals.alloc(local("failure", exception_ty));
    h.user_fn_full(
        "origin",
        Vec::new(),
        vec![param("failure", exception_ty, input)],
        h.unit,
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Throw(local_ref(
                input,
                exception_ty,
            )))],
        },
    );
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));
    let (id, origin) = module
        .functions
        .iter()
        .find(|(_, f)| f.name == "origin")
        .unwrap();
    let block = origin
        .body
        .blocks
        .iter()
        .find_map(|(_, block)| {
            matches!(block.terminator, mir::Terminator::Throw { .. }).then_some(block)
        })
        .unwrap();
    let mir::Terminator::Throw { exception, .. } = &block.terminator else {
        unreachable!()
    };
    let mir::StatementKind::Call(mir::CallEffect::Value { destination, call }) =
        &block.statements.last().unwrap().kind
    else {
        panic!("throw must prepare before unwinding")
    };
    let mir::CallTarget::Direct(mir::Callee::Runtime(mir::RuntimeFn::PrepareThrowable {
        trace_bytes,
    })) = call.target
    else {
        panic!("the preparation must be a managed runtime call")
    };
    assert!(matches!(exception.kind, mir::ExprKind::Local(local) if local == *destination));
    assert!(matches!(call.args[0].kind, mir::ExprKind::Local(local) if local != *destination));
    assert_eq!(
        mir::array_type(&module, &mir::Type::Class(trace_bytes)),
        Some((
            mir::ArrayKind::Immutable,
            &mir::Type::Integer(mir::IntegerKind::UNSIGNED_8)
        ))
    );
    assert!(
        module
            .meta
            .local_values
            .get_owned(id.into(), *destination)
            .is_some()
    );
}
