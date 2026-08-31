//! Reifies native EH state before a coroutine can leave its native stack.

use scoop_ast::Span;
use scoop_mir as mir;

pub(super) fn materialize_exceptions(body: &mut mir::Body, throwable: mir::Type) {
    if !body.blocks.iter().any(|(_, block)| {
        block.statements.iter().any(|statement| {
            matches!(
                statement.kind,
                mir::StatementKind::Eh(mir::EhStatement::LandingPad { .. })
            )
        })
    }) {
        return;
    }

    let managed = body.locals.alloc(mir::Local {
        name: "$coroutine_exception".to_string(),
        ty: throwable,
        mutable: false,
    });

    for (_, block) in body.blocks.iter_mut() {
        let old = std::mem::take(&mut block.statements);
        for mut statement in old {
            match statement.kind {
                mir::StatementKind::Eh(mir::EhStatement::LandingPad { .. }) => {
                    statement.kind =
                        mir::StatementKind::Eh(mir::EhStatement::LandingPad { cleanup: false });
                    let span = statement.span;
                    block.statements.push(statement);
                    block
                        .statements
                        .push(eh(mir::EhStatement::BeginCatch, span));
                    block.statements.push(mir::Statement {
                        kind: mir::StatementKind::Call(mir::CallEffect::Value {
                            destination: managed,
                            call: mir::Call {
                                target: mir::CallTarget {
                                    kind: mir::CallKind::Direct,
                                    callee: mir::Callee::Runtime(
                                        mir::RuntimeFn::MaterializeException,
                                    ),
                                },
                                args: vec![mir::Expr::CaughtException],
                            },
                        }),
                        span,
                    });
                    block.statements.push(eh(mir::EhStatement::EndCatch, span));
                }
                mir::StatementKind::Eh(
                    mir::EhStatement::BeginCatch | mir::EhStatement::EndCatch,
                ) => {}
                _ => {
                    rewrite_statement(&mut statement.kind, managed);
                    block.statements.push(statement);
                }
            }
        }

        rewrite_terminator(&mut block.terminator, managed, block.unwind);
    }
}

fn eh(kind: mir::EhStatement, span: Span) -> mir::Statement {
    mir::Statement {
        kind: mir::StatementKind::Eh(kind),
        span,
    }
}

fn rewrite_statement(statement: &mut mir::StatementKind, managed: mir::LocalId) {
    match statement {
        mir::StatementKind::Expr(expr) => rewrite_expr(expr, managed),
        mir::StatementKind::ValDecl { init, .. } => rewrite_expr(init, managed),
        mir::StatementKind::Assign { value, .. } => rewrite_expr(value, managed),
        mir::StatementKind::Call(effect) => match effect {
            mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => {
                for arg in &mut call.args {
                    rewrite_expr(arg, managed);
                }
            }
        },
        mir::StatementKind::ArraySet {
            array,
            index,
            value,
        } => {
            rewrite_expr(array, managed);
            rewrite_expr(index, managed);
            rewrite_expr(value, managed);
        }
        mir::StatementKind::FieldSet { object, value, .. } => {
            rewrite_expr(object, managed);
            rewrite_expr(value, managed);
        }
        mir::StatementKind::Eh(_) => {}
    }
}

fn rewrite_terminator(
    terminator: &mut mir::Terminator,
    managed: mir::LocalId,
    block_unwind: Option<mir::BlockId>,
) {
    match terminator {
        mir::Terminator::Branch { cond, .. } => rewrite_expr(cond, managed),
        mir::Terminator::Return { value } => {
            if let Some(value) = value {
                rewrite_expr(value, managed);
            }
        }
        mir::Terminator::Throw { exception, .. } => rewrite_expr(exception, managed),
        mir::Terminator::Rethrow { unwind } => {
            *terminator = mir::Terminator::Throw {
                exception: mir::Expr::Local(managed),
                unwind: *unwind,
            };
        }
        mir::Terminator::Resume => {
            *terminator = mir::Terminator::Throw {
                exception: mir::Expr::Local(managed),
                unwind: block_unwind,
            };
        }
        mir::Terminator::Goto(_) | mir::Terminator::Trap { .. } | mir::Terminator::Unreachable => {}
    }
}

fn rewrite_expr(expr: &mut mir::Expr, managed: mir::LocalId) {
    match expr {
        mir::Expr::CaughtException => *expr = mir::Expr::Local(managed),
        mir::Expr::TupleLiteral(elements)
        | mir::Expr::ArrayLiteral(elements)
        | mir::Expr::StructInit { args: elements, .. }
        | mir::Expr::ClassInit { args: elements, .. }
        | mir::Expr::ClosureAlloc {
            captures: elements, ..
        }
        | mir::Expr::VariantConstruct {
            fields: elements, ..
        } => {
            for element in elements {
                rewrite_expr(element, managed);
            }
        }
        mir::Expr::Retype { operand, .. }
        | mir::Expr::ClosureCapture {
            closure: operand, ..
        }
        | mir::Expr::FieldAccess {
            receiver: operand, ..
        }
        | mir::Expr::Box(operand)
        | mir::Expr::Unbox(operand)
        | mir::Expr::IsInstance { operand, .. }
        | mir::Expr::Cast { operand, .. }
        | mir::Expr::ArrayLen(operand)
        | mir::Expr::ArrayClone(operand)
        | mir::Expr::PtrFromUInt { operand, .. }
        | mir::Expr::PtrToUInt(operand)
        | mir::Expr::PtrCast { operand, .. }
        | mir::Expr::Unary { operand, .. }
        | mir::Expr::EnumTag(operand)
        | mir::Expr::EnumField { operand, .. } => rewrite_expr(operand, managed),
        mir::Expr::ArrayGet { array, index }
        | mir::Expr::Binary {
            lhs: array,
            rhs: index,
            ..
        } => {
            rewrite_expr(array, managed);
            rewrite_expr(index, managed);
        }
        mir::Expr::PtrLoad {
            pointer, offset, ..
        } => {
            rewrite_expr(pointer, managed);
            if let Some(offset) = offset {
                rewrite_expr(offset, managed);
            }
        }
        mir::Expr::PtrStore {
            pointer,
            offset,
            value,
            ..
        } => {
            rewrite_expr(pointer, managed);
            if let Some(offset) = offset {
                rewrite_expr(offset, managed);
            }
            rewrite_expr(value, managed);
        }
        mir::Expr::PtrOffset {
            pointer, offset, ..
        } => {
            rewrite_expr(pointer, managed);
            rewrite_expr(offset, managed);
        }
        mir::Expr::StringConst(_)
        | mir::Expr::IntLiteral(_)
        | mir::Expr::BoolLiteral(_)
        | mir::Expr::UnitLiteral
        | mir::Expr::Local(_)
        | mir::Expr::AddressOf { .. }
        | mir::Expr::SizeOf(_)
        | mir::Expr::AlignOf(_)
        | mir::Expr::FunPtrNull(_)
        | mir::Expr::FunctionAddress { .. } => {}
    }
}
