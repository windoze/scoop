//! Reuse allocation zeroing only for a directly known, effect-free initializer.

use super::*;

impl BodyLowerer<'_> {
    pub(in crate::body) fn is_zero_array_initializer(&self, initializer: &hir::Expr) -> bool {
        let function = match initializer.kind {
            hir::ExprKind::Local(local) => {
                return self
                    .zero_array_initializers
                    .contains(&self.local_map[&local]);
            }
            hir::ExprKind::Lambda(id) => self.module.lambdas[id].function,
            hir::ExprKind::AnonymousFunction(id) => self.module.anonymous_functions[id].function,
            _ => return false,
        };
        let hir::FunctionKind::User(body) = &self.module.functions[function].kind else {
            return false;
        };
        let [statement] = body.statements.as_slice() else {
            return false;
        };
        match &statement.kind {
            hir::StatementKind::Return { value } => zero_value(value),
            _ => false,
        }
    }
}

fn zero_value(value: &hir::Expr) -> bool {
    match &value.kind {
        hir::ExprKind::MaybeUninit(hir::MaybeUninitOperation::Uninit)
        | hir::ExprKind::BoolLiteral(false)
        | hir::ExprKind::UnitLiteral => true,
        hir::ExprKind::IntegerLiteral(value) => value.raw_bits() == 0,
        hir::ExprKind::MaybeUninit(hir::MaybeUninitOperation::Initialized(value)) => {
            zero_value(value)
        }
        hir::ExprKind::TupleLiteral(values) => values.iter().all(zero_value),
        _ => false,
    }
}
