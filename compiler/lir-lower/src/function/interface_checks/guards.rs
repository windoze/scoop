//! Follow a branch's local Boolean copies without moving the original check.

use super::*;

pub(super) fn check(block: &mir::BasicBlock, addressed: &HashSet<mir::LocalId>) -> Option<Check> {
    let mir::Terminator::Branch {
        cond,
        then_block,
        else_block,
    } = &block.terminator
    else {
        return None;
    };
    if then_block == else_block {
        return None;
    }
    let mut expression = cond;
    let mut before = block.statements.len();
    let mut location = Location::Terminator;
    let mut negated = false;
    loop {
        if let Some((operand, interface, inverted)) = condition(expression) {
            let local = interface_tables::source_local(operand)?;
            if addressed.contains(&local)
                || block.statements[before..]
                    .iter()
                    .any(|statement| interface_tables::written_local(statement) == Some(local))
            {
                return None;
            }
            return Some(Check {
                source: (local, interface),
                success: if negated ^ inverted {
                    *else_block
                } else {
                    *then_block
                },
                location,
            });
        }
        match &expression.kind {
            mir::ExprKind::Unary {
                op: mir::UnOp::BoolNot,
                operand,
            } => {
                negated = !negated;
                expression = operand;
            }
            mir::ExprKind::Local(local) if !addressed.contains(local) => {
                let index = block.statements[..before].iter().rposition(|statement| {
                    interface_tables::written_local(statement) == Some(*local)
                })?;
                expression = match &block.statements[index].kind {
                    mir::StatementKind::ValDecl { init, .. } => init,
                    mir::StatementKind::Assign { value, .. } => value,
                    _ => return None,
                };
                location = Location::Statement(index);
                before = index;
            }
            _ => return None,
        }
    }
}
