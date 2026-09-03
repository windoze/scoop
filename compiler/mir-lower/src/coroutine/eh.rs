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
        ty: throwable.clone(),
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
                                args: vec![mir::Expr::new(
                                    mir::Type::Any,
                                    mir::ExprKind::CaughtException,
                                )],
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
                    rewrite_statement(&mut statement.kind, managed, &throwable);
                    block.statements.push(statement);
                }
            }
        }

        rewrite_terminator(&mut block.terminator, managed, &throwable, block.unwind);
    }
}

fn eh(kind: mir::EhStatement, span: Span) -> mir::Statement {
    mir::Statement {
        kind: mir::StatementKind::Eh(kind),
        span,
    }
}

fn rewrite_statement(
    statement: &mut mir::StatementKind,
    managed: mir::LocalId,
    managed_ty: &mir::Type,
) {
    match statement {
        mir::StatementKind::Expr(expr) => rewrite_expr(expr, managed, managed_ty),
        mir::StatementKind::ValDecl { init, .. } => rewrite_expr(init, managed, managed_ty),
        mir::StatementKind::Assign { value, .. } => rewrite_expr(value, managed, managed_ty),
        mir::StatementKind::GlobalAssign { value, .. } => rewrite_expr(value, managed, managed_ty),
        mir::StatementKind::Call(effect) => match effect {
            mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => {
                for arg in &mut call.args {
                    rewrite_expr(arg, managed, managed_ty);
                }
            }
        },
        mir::StatementKind::ArraySet {
            array,
            index,
            value,
            ..
        } => {
            rewrite_expr(array, managed, managed_ty);
            rewrite_expr(index, managed, managed_ty);
            rewrite_expr(value, managed, managed_ty);
        }
        mir::StatementKind::FieldSet { object, value, .. }
        | mir::StatementKind::AtomicFieldStore { object, value, .. } => {
            rewrite_expr(object, managed, managed_ty);
            rewrite_expr(value, managed, managed_ty);
        }
        mir::StatementKind::Eh(_) => {}
    }
}

fn rewrite_terminator(
    terminator: &mut mir::Terminator,
    managed: mir::LocalId,
    managed_ty: &mir::Type,
    block_unwind: Option<mir::BlockId>,
) {
    match terminator {
        mir::Terminator::Branch { cond, .. } => rewrite_expr(cond, managed, managed_ty),
        mir::Terminator::Return { value } => {
            if let Some(value) = value {
                rewrite_expr(value, managed, managed_ty);
            }
        }
        mir::Terminator::Throw { exception, .. } => rewrite_expr(exception, managed, managed_ty),
        mir::Terminator::Rethrow { unwind } => {
            *terminator = mir::Terminator::Throw {
                exception: mir::Expr::new(managed_ty.clone(), mir::ExprKind::Local(managed)),
                unwind: *unwind,
            };
        }
        mir::Terminator::Resume => {
            *terminator = mir::Terminator::Throw {
                exception: mir::Expr::new(managed_ty.clone(), mir::ExprKind::Local(managed)),
                unwind: block_unwind,
            };
        }
        mir::Terminator::Goto(_) | mir::Terminator::Trap { .. } | mir::Terminator::Unreachable => {}
    }
}

fn rewrite_expr(expr: &mut mir::Expr, managed: mir::LocalId, managed_ty: &mir::Type) {
    match &mut expr.kind {
        mir::ExprKind::CaughtException => {
            *expr = mir::Expr::new(managed_ty.clone(), mir::ExprKind::Local(managed));
        }
        mir::ExprKind::TupleLiteral(elements)
        | mir::ExprKind::ArrayLiteral { elements, .. }
        | mir::ExprKind::StructInit { args: elements, .. }
        | mir::ExprKind::ClosureAlloc {
            captures: elements, ..
        }
        | mir::ExprKind::VariantConstruct {
            fields: elements, ..
        } => {
            for element in elements {
                rewrite_expr(element, managed, managed_ty);
            }
        }
        mir::ExprKind::ArrayAssembly { parts, .. } => {
            for part in parts {
                match part {
                    mir::ArrayAssemblyPart::Element(value)
                    | mir::ArrayAssemblyPart::CopyArray(value) => {
                        rewrite_expr(value, managed, managed_ty)
                    }
                }
            }
        }
        mir::ExprKind::Retype { operand, .. }
        | mir::ExprKind::ClosureCapture {
            closure: operand, ..
        }
        | mir::ExprKind::ForeignCallbackRegister {
            closure: operand, ..
        }
        | mir::ExprKind::ForeignCallbackOperation {
            callback: operand, ..
        }
        | mir::ExprKind::FieldAccess {
            receiver: operand, ..
        }
        | mir::ExprKind::AtomicFieldLoad {
            object: operand, ..
        }
        | mir::ExprKind::Box(operand)
        | mir::ExprKind::Unbox(operand)
        | mir::ExprKind::IsInstance { operand, .. }
        | mir::ExprKind::Cast { operand, .. }
        | mir::ExprKind::ArrayLen { operand, .. }
        | mir::ExprKind::ArrayClone { operand, .. }
        | mir::ExprKind::PtrFromUInt { operand, .. }
        | mir::ExprKind::PtrToUInt(operand)
        | mir::ExprKind::PtrCast { operand, .. }
        | mir::ExprKind::Unary { operand, .. }
        | mir::ExprKind::EnumTag(operand)
        | mir::ExprKind::EnumField { operand, .. } => rewrite_expr(operand, managed, managed_ty),
        mir::ExprKind::AtomicFieldCompareExchange {
            object,
            expected,
            replacement,
            ..
        } => {
            rewrite_expr(object, managed, managed_ty);
            rewrite_expr(expected, managed, managed_ty);
            rewrite_expr(replacement, managed, managed_ty);
        }
        mir::ExprKind::ArrayGet { array, index, .. }
        | mir::ExprKind::Binary {
            lhs: array,
            rhs: index,
            ..
        } => {
            rewrite_expr(array, managed, managed_ty);
            rewrite_expr(index, managed, managed_ty);
        }
        mir::ExprKind::PtrLoad {
            pointer, offset, ..
        } => {
            rewrite_expr(pointer, managed, managed_ty);
            if let Some(offset) = offset {
                rewrite_expr(offset, managed, managed_ty);
            }
        }
        mir::ExprKind::PtrStore {
            pointer,
            offset,
            value,
            ..
        } => {
            rewrite_expr(pointer, managed, managed_ty);
            if let Some(offset) = offset {
                rewrite_expr(offset, managed, managed_ty);
            }
            rewrite_expr(value, managed, managed_ty);
        }
        mir::ExprKind::PtrOffset {
            pointer, offset, ..
        } => {
            rewrite_expr(pointer, managed, managed_ty);
            rewrite_expr(offset, managed, managed_ty);
        }
        mir::ExprKind::ClassAlloc { .. }
        | mir::ExprKind::StringConst(_)
        | mir::ExprKind::IntLiteral(_)
        | mir::ExprKind::BoolLiteral(_)
        | mir::ExprKind::UnitLiteral
        | mir::ExprKind::Local(_)
        | mir::ExprKind::GlobalRead(_)
        | mir::ExprKind::AddressOf { .. }
        | mir::ExprKind::GlobalAddress { .. }
        | mir::ExprKind::SizeOf(_)
        | mir::ExprKind::AlignOf(_)
        | mir::ExprKind::FunPtrNull(_)
        | mir::ExprKind::FunctionAddress { .. } => {}
    }
}
