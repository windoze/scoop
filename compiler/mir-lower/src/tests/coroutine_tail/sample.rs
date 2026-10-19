use super::*;

#[derive(Clone, Copy)]
pub(super) enum Shape {
    Exact,
    Add,
    EarlierCall,
    TailThenAdd,
}

pub(super) fn lower_tail(source: &hir::ExportHirOutput, enabled: bool) -> mir::Module {
    let concrete = scoop_hir_lower::concretize_output(source).unwrap();
    crate::lower_with_dependencies(
        &concrete,
        Arena::new(),
        HashMap::new(),
        Vec::new(),
        &[],
        enabled,
    )
}

pub(super) fn program(shape: Shape, depth: usize, unit: bool) -> hir::ExportHirOutput {
    let mut h = Harness::new();
    let result = if unit { h.unit } else { h.int };
    let value = if unit {
        expr(hir::ExprKind::UnitLiteral, result)
    } else {
        int_lit(&h, 42)
    };
    let mut target = h.user_fn_full(
        "leaf",
        Vec::new(),
        Vec::new(),
        result,
        hir::Body {
            locals: Arena::new(),
            statements: vec![stmt(hir::StatementKind::Return { value })],
        },
    );
    h.functions[target].is_suspend = true;
    for index in 0..depth {
        let mut locals = Arena::new();
        let mut statements = Vec::new();
        if matches!(shape, Shape::EarlierCall) {
            let discarded = locals.alloc(local("discarded", result));
            statements.push(val_decl(discarded, call_typed(target, Vec::new(), result)));
        }
        let called = call_typed(target, Vec::new(), result);
        let value = if matches!(shape, Shape::Add)
            || (matches!(shape, Shape::TailThenAdd) && index + 1 == depth)
        {
            let increment = int_lit(&h, 1);
            integer_binary(
                &mut h,
                hir::IntegerKind::SIGNED_32,
                hir::IntegerBinaryOperator::Add,
                called,
                increment,
            )
        } else {
            called
        };
        statements.push(stmt(hir::StatementKind::Return { value }));
        target = h.user_fn_full(
            &format!("wrapper{index}"),
            Vec::new(),
            Vec::new(),
            result,
            hir::Body { locals, statements },
        );
        h.functions[target].is_suspend = true;
    }
    let main = empty_main(&mut h);
    h.finish_coroutines(main)
}
