//! Operations with no managed-throw semantics, independently of GC or writes.

use super::mir;

pub(super) fn statement(statement: &mir::Statement) -> bool {
    let mut safe = matches!(
        statement.kind,
        mir::StatementKind::Expr(_)
            | mir::StatementKind::ValDecl { .. }
            | mir::StatementKind::Assign { .. }
            | mir::StatementKind::FieldSet { .. }
            | mir::StatementKind::Call(_)
            | mir::StatementKind::Eh(_)
    );
    mir::visit_statement_exprs(statement, &mut |expr| safe &= expression(expr));
    safe
}

pub(super) fn terminator(terminator: &mir::Terminator) -> bool {
    let mut safe = matches!(
        terminator,
        mir::Terminator::Goto(_)
            | mir::Terminator::Branch { .. }
            | mir::Terminator::Return { .. }
            | mir::Terminator::Trap { .. }
            | mir::Terminator::Unreachable
    );
    mir::visit_terminator_exprs(terminator, &mut |expr| safe &= expression(expr));
    safe
}

fn expression(expression: &mir::Expr) -> bool {
    use mir::ExprKind as E;
    matches!(
        expression.kind,
        E::Local(_)
            | E::IntegerLiteral(_)
            | E::MachineScalarLiteral(_)
            | E::BoolLiteral(_)
            | E::CharLiteral(_)
            | E::FloatLiteral(_)
            | E::UnitLiteral
            | E::IntegerUnary { .. }
            | E::IntegerBinary { .. }
            | E::IntegerCompare { .. }
            | E::IntegerCompareTo { .. }
            | E::IntegerShift { .. }
            | E::IntegerConversion { .. }
            | E::FloatUnary { .. }
            | E::FloatBinary { .. }
            | E::FloatConversion { .. }
            | E::CharCode(_)
            | E::CharFromCodeUnchecked(_)
            | E::Unary { .. }
            | E::Binary { .. }
            | E::Retype { .. }
            | E::FieldAccess { .. }
            | E::ClosureCapture { .. }
            | E::CaughtException
    )
}
