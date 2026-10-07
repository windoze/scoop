//! Scoped borrows reuse the ordinary closure ABI and exception cleanup.

use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_data_borrow(
        &mut self,
        kind: hir::DataBorrowIntrinsic,
        args: &[hir::Expr],
        result_ty: hir::TypeId,
    ) -> smir::Expr {
        use mir::DataBorrowOperationKind as Op;
        let [object, block] = args else {
            unreachable!("a checked data borrow has an object and a callback")
        };
        let span = object.span;
        let object = self.borrow_argument(object, "borrow_object");
        let block = self.borrow_argument(block, "borrow_block");
        let mir::Type::Function(function_type) = block.ty else {
            unreachable!("a checked data borrow has an ordinary function callback")
        };
        let source = match kind {
            hir::DataBorrowIntrinsic::String => mir::BorrowDataSource::String,
            hir::DataBorrowIntrinsic::Array | hir::DataBorrowIntrinsic::MutableArray => {
                let mir::Type::Class(class) = object.ty else {
                    unreachable!("a checked array borrow has an exact array class")
                };
                mir::BorrowDataSource::Array(class)
            }
        };
        let frame_ty = mir::Type::Ptr(Box::new(mir::Type::Unit));
        let frame = self.new_hidden("pin_frame", frame_ty.clone(), false);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: frame,
            init: borrow_operation(frame_ty.clone(), Op::PushPinFrame, object.clone()),
        });
        let result_ty = self.lower_type(result_ty);
        let result = self.new_hidden("borrow_result", result_ty.clone(), false);
        let pointer_ty = self.shell.function_types[function_type].parameter_types[0].clone();
        let call = smir::Expr::new(
            result_ty.clone(),
            smir::ExprKind::Call(smir::Call {
                target: mir::CallTarget {
                    kind: mir::CallKind::Closure { function_type },
                    callee: mir::Callee::Closure(function_type),
                },
                args: vec![
                    block,
                    borrow_operation(pointer_ty, Op::DataPointer(source), object.clone()),
                    borrow_operation(
                        mir::Type::Integer(mir::IntegerKind::SIGNED_64),
                        Op::Length(source),
                        object,
                    ),
                ],
                return_ty: result_ty.clone(),
            }),
        );
        self.prelude.push(smir::StatementKind::Try(smir::Try {
            body: vec![smir::Statement {
                span,
                kind: smir::StatementKind::ValDecl {
                    local: result,
                    init: call,
                },
            }],
            catches: Vec::new(),
            finally_body: Some(vec![smir::Statement {
                span,
                kind: smir::StatementKind::Expr(borrow_operation(
                    mir::Type::Unit,
                    Op::PopPinFrame,
                    smir::Expr::local(frame, frame_ty),
                )),
            }]),
        }));
        smir::Expr::local(result, result_ty)
    }

    fn borrow_argument(&mut self, source: &hir::Expr, name: &str) -> smir::Expr {
        let init = self.lower_expr(source);
        let ty = init.ty.clone();
        let local = self.new_hidden(name, ty.clone(), false);
        self.prelude
            .push(smir::StatementKind::ValDecl { local, init });
        smir::Expr::local(local, ty)
    }
}

fn borrow_operation(
    ty: mir::Type,
    kind: mir::DataBorrowOperationKind,
    operand: smir::Expr,
) -> smir::Expr {
    smir::Expr::new(
        ty,
        smir::ExprKind::DataBorrow(mir::DataBorrowOperation {
            kind,
            operand: Box::new(operand),
        }),
    )
}
