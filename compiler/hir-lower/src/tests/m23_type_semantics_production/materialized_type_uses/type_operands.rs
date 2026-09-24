use super::*;
use hir::concrete::{FunctionKind, Statement, StatementKind};

pub(super) fn with_layout_operations(
    local: &hir::LocalConcreteHirOutput,
) -> hir::LocalConcreteHirOutput {
    let mut module = local.module().clone();
    let integer = |kind| {
        module
            .types
            .iter()
            .find(|(_, ty)| ty.kind == TypeKind::Integer(kind))
            .unwrap()
            .0
    };
    let size_target = integer(hir::IntegerKind::UNSIGNED_16);
    let align_target = integer(hir::IntegerKind::UNSIGNED_8);
    let result = integer(hir::IntegerKind::UNSIGNED_64);
    let (_, function) = module
        .functions
        .iter_mut()
        .find(|(_, function)| function.name == "primitiveTest")
        .unwrap();
    let FunctionKind::User(body) = &mut function.kind else {
        panic!("materialized source body")
    };
    let StatementKind::Return {
        value: Some(expression),
    } = &body.statements[0].kind
    else {
        panic!("source type test")
    };
    let expression = expression.clone();
    // Exercise normalized HIR directly: importing generic sizeOf/alignOf
    // through an ordinary dependency retains the separate M23-7 gate.
    for kind in [
        ExprKind::SizeOf(size_target),
        ExprKind::AlignOf(align_target),
    ] {
        body.statements.insert(
            0,
            Statement {
                span: expression.span,
                kind: StatementKind::Expr(hir::concrete::Expr {
                    kind,
                    ty: result,
                    ..expression.clone()
                }),
            },
        );
    }
    hir::LocalConcreteHirOutput::try_new(
        module,
        local.output_kind().clone(),
        local.materialization().clone(),
    )
    .unwrap()
}
