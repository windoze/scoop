//! Selection of portable ordinary bodies for the existing MIR inliner.

use crate::*;

/// Unsupported bodies keep their ordinary external entry. This is an
/// optimization selection, not a restriction on valid source programs.
pub fn portable_inline_body(signature: &CallableSignature, kind: &FunctionKind) -> bool {
    let FunctionKind::User(body) = kind else {
        return false;
    };
    if signature.is_suspend || !signature.context_parameters.is_empty() {
        return false;
    }
    statements(&body.statements)
}

fn statements(body: &[Statement]) -> bool {
    body.iter().all(|statement| match &statement.kind {
        StatementKind::Expr(value)
        | StatementKind::Return { value }
        | StatementKind::Throw(value) => expression(value),
        StatementKind::ValDecl {
            pattern: Pattern::Binding { .. } | Pattern::Wildcard,
            init,
        } => expression(init),
        StatementKind::Assign {
            target: AssignTarget::Local(_),
            value,
        } => expression(value),
        StatementKind::Assign {
            target: AssignTarget::Field { receiver, .. },
            value,
        } => expression(receiver) && expression(value),
        StatementKind::If {
            cond,
            then_body,
            else_body,
        } => {
            expression(cond)
                && statements(then_body)
                && else_body.as_ref().is_none_or(|body| statements(body))
        }
        StatementKind::While {
            condition_setup,
            cond,
            body,
            ..
        } => statements(condition_setup) && expression(cond) && statements(body),
        StatementKind::Break { .. } | StatementKind::Continue { .. } => true,
        _ => false,
    })
}

fn expression(value: &Expr) -> bool {
    use ExprKind as E;
    match &value.kind {
        E::Local(_)
        | E::IntegerLiteral(_)
        | E::FloatLiteral(_)
        | E::BoolLiteral(_)
        | E::CharLiteral(_)
        | E::StringLiteral {
            owner: StringConstantOwner::CurrentDefinition,
            ..
        }
        | E::NoneLiteral
        | E::UnitLiteral
        | E::AddressOf(Place::Local(_))
        | E::SizeOf(_)
        | E::AlignOf(_)
        | E::Unreachable => true,
        E::TupleLiteral(values)
        | E::StructInit { args: values, .. }
        | E::ClassInit { args: values, .. }
        | E::VariantConstruct { args: values, .. }
        | E::StructConstruct { fields: values, .. } => values.iter().all(expression),
        E::IntegerOperation(operation) => match operation {
            IntegerExpression::Unary { operand, .. } => expression(operand),
            IntegerExpression::Binary { lhs, rhs, .. } => expression(lhs) && expression(rhs),
        },
        E::Binary { lhs, rhs, .. }
        | E::FloatBinary { lhs, rhs, .. }
        | E::PrimitiveBinary { lhs, rhs, .. } => expression(lhs) && expression(rhs),
        E::Unary { operand, .. }
        | E::FloatUnary { operand, .. }
        | E::PrimitiveUnary { operand, .. }
        | E::IntegerConversion { operand, .. }
        | E::FloatConversion { operand, .. }
        | E::CharCode(operand)
        | E::CharFromCodeUnchecked(operand)
        | E::PtrCast(operand)
        | E::PtrToULong(operand)
        | E::PtrFromNonZeroULong(operand)
        | E::SomeWrap(operand)
        | E::FieldAccess {
            receiver: operand, ..
        } => expression(operand),
        E::PtrLoad { pointer, offset } => {
            expression(pointer) && offset.as_ref().is_none_or(|value| expression(value))
        }
        E::PtrStore {
            pointer,
            offset,
            value,
        } => {
            expression(pointer)
                && offset.as_ref().is_none_or(|value| expression(value))
                && expression(value)
        }
        E::PtrOffset {
            pointer, offset, ..
        } => expression(pointer) && expression(offset),
        E::Call { callee, args, .. } => ordinary_target(*callee) && args.iter().all(expression),
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
                && expression(receiver)
                && args.iter().all(expression)
        }
        _ => false,
    }
}

fn ordinary_target(target: CallableTarget) -> bool {
    matches!(
        target,
        CallableTarget::Local(Callable::Function(_))
            | CallableTarget::Dependency(_)
            | CallableTarget::Application(_)
    )
}
