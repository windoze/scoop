//! Selection of portable ordinary bodies for the existing MIR inliner.

use crate::*;

/// Unsupported bodies keep their ordinary external entry. This is an
/// optimization selection, not a restriction on valid source programs.
pub fn small_inline_body(signature: &CallableSignature, kind: &FunctionKind) -> bool {
    let FunctionKind::User(body) = kind else {
        return false;
    };
    if signature.is_suspend || !signature.context_parameters.is_empty() {
        return false;
    }
    let mut remaining = 96usize;
    statements(&body.statements, &mut remaining)
}

fn statements(body: &[Statement], remaining: &mut usize) -> bool {
    body.iter().all(|statement| {
        if !take(remaining) {
            return false;
        }
        match &statement.kind {
            StatementKind::Expr(value)
            | StatementKind::Return { value }
            | StatementKind::Throw(value) => expression(value, remaining),
            StatementKind::ValDecl {
                pattern: Pattern::Binding { .. } | Pattern::Wildcard,
                init,
            } => expression(init, remaining),
            StatementKind::Assign {
                target: AssignTarget::Local(_),
                value,
            } => expression(value, remaining),
            StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                expression(cond, remaining)
                    && statements(then_body, remaining)
                    && else_body
                        .as_ref()
                        .is_none_or(|body| statements(body, remaining))
            }
            _ => false,
        }
    })
}

fn expression(value: &Expr, remaining: &mut usize) -> bool {
    use ExprKind as E;
    if !take(remaining) {
        return false;
    }
    match &value.kind {
        E::Local(_)
        | E::IntegerLiteral(_)
        | E::FloatLiteral(_)
        | E::BoolLiteral(_)
        | E::CharLiteral(_)
        | E::UnitLiteral
        | E::Unreachable => true,
        E::TupleLiteral(values) => values.iter().all(|value| expression(value, remaining)),
        E::IntegerOperation(operation) => match operation {
            IntegerExpression::Unary { operand, .. } => expression(operand, remaining),
            IntegerExpression::Binary { lhs, rhs, .. } => {
                expression(lhs, remaining) && expression(rhs, remaining)
            }
        },
        E::Binary { lhs, rhs, .. }
        | E::FloatBinary { lhs, rhs, .. }
        | E::PrimitiveBinary { lhs, rhs, .. } => {
            expression(lhs, remaining) && expression(rhs, remaining)
        }
        E::Unary { operand, .. }
        | E::FloatUnary { operand, .. }
        | E::PrimitiveUnary { operand, .. }
        | E::IntegerConversion { operand, .. }
        | E::FloatConversion { operand, .. }
        | E::CharCode(operand)
        | E::CharFromCodeUnchecked(operand)
        | E::FieldAccess {
            receiver: operand, ..
        } => expression(operand, remaining),
        E::Call { callee, args, .. } => {
            ordinary_target(*callee) && args.iter().all(|value| expression(value, remaining))
        }
        E::MethodCall {
            receiver,
            args,
            callee,
        }
        | E::DirectSuperMethodCall {
            receiver,
            args,
            callee,
        } => {
            matches!(callee, MethodCallee::Callable(target) if ordinary_target(*target))
                && expression(receiver, remaining)
                && args.iter().all(|value| expression(value, remaining))
        }
        _ => false,
    }
}

fn take(remaining: &mut usize) -> bool {
    match remaining.checked_sub(1) {
        Some(next) => {
            *remaining = next;
            true
        }
        None => false,
    }
}

fn ordinary_target(target: CallableTarget) -> bool {
    matches!(
        target,
        CallableTarget::Local(Callable::Function(_)) | CallableTarget::Dependency(_)
    )
}
