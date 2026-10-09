use super::*;

/// Mutably visit expression trees in their statement evaluation order.
pub fn visit_statement_exprs_mut(statement: &mut Statement, visitor: &mut impl FnMut(&mut Expr)) {
    match &mut statement.kind {
        StatementKind::Expr(expr) => visit_expr_mut(expr, visitor),
        StatementKind::Call(effect) => {
            let call = match effect {
                CallEffect::Unit(call) | CallEffect::Value { call, .. } => call,
            };
            for argument in &mut call.args {
                visit_expr_mut(argument, visitor);
            }
        }
        StatementKind::ValDecl { init, .. } => visit_expr_mut(init, visitor),
        StatementKind::PublishReleaseReady { receiver, .. } => visit_expr_mut(receiver, visitor),
        StatementKind::Assign { value, .. } | StatementKind::GlobalAssign { value, .. } => {
            visit_expr_mut(value, visitor);
        }
        StatementKind::ArraySet {
            array,
            index,
            value,
            ..
        } => {
            visit_expr_mut(array, visitor);
            visit_expr_mut(index, visitor);
            visit_expr_mut(value, visitor);
        }
        StatementKind::FieldSet { object, value, .. }
        | StatementKind::AtomicFieldStore { object, value, .. } => {
            visit_expr_mut(object, visitor);
            visit_expr_mut(value, visitor);
        }
        StatementKind::Eh(_) => {}
    }
}

pub fn visit_terminator_exprs_mut(
    terminator: &mut Terminator,
    visitor: &mut impl FnMut(&mut Expr),
) {
    match terminator {
        Terminator::Branch { cond, .. } => visit_expr_mut(cond, visitor),
        Terminator::Return { value: Some(value) } => visit_expr_mut(value, visitor),
        Terminator::Throw { exception, .. } => visit_expr_mut(exception, visitor),
        Terminator::Goto(_)
        | Terminator::Return { value: None }
        | Terminator::Rethrow { .. }
        | Terminator::Resume
        | Terminator::Trap { .. }
        | Terminator::Unreachable => {}
    }
}

pub fn visit_block_exprs_mut(block: &mut BasicBlock, visitor: &mut impl FnMut(&mut Expr)) {
    for statement in &mut block.statements {
        visit_statement_exprs_mut(statement, visitor);
    }
    visit_terminator_exprs_mut(&mut block.terminator, visitor);
}
