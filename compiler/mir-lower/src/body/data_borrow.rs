//! Scoped borrows reuse the ordinary closure ABI and exception cleanup.

use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_data_borrow(
        &mut self,
        borrow: &hir::DataBorrow,
        result_ty: hir::TypeId,
    ) -> smir::Expr {
        use mir::DataBorrowOperationKind as Op;
        let span = borrow.object.span;
        let object = self.borrow_argument(&borrow.object, "borrow_object");
        let block = self.borrow_argument(&borrow.block, "borrow_block");
        let function_type = self.lower_function_type_id(borrow.function_type);
        let source = match borrow.source {
            hir::BorrowDataSource::String => mir::BorrowDataSource::String,
            hir::BorrowDataSource::Array(class) => {
                mir::BorrowDataSource::Array(self.class_map[&class])
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
                target: mir::CallTarget::Closure {
                    function_type,
                    receiver: Box::new(block),
                },
                args: vec![
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
                debug_origin: Default::default(),
                span,
                kind: smir::StatementKind::ValDecl {
                    local: result,
                    init: call,
                },
            }],
            catches: Vec::new(),
            finally_body: Some(vec![smir::Statement {
                debug_origin: Default::default(),
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
