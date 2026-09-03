use super::*;

impl<'a> CfgLowerer<'a> {
    /// Normalize an expression into a call-free MIR expression, emitting
    /// calls in source evaluation order as explicit effect statements.
    pub(super) fn lower_expr(&mut self, expr: &smir::Expr, span: Span) -> mir::Expr {
        let kind = match &expr.kind {
            smir::ExprKind::StringConst(id) => mir::ExprKind::StringConst(*id),
            smir::ExprKind::IntLiteral(value) => mir::ExprKind::IntLiteral(*value),
            smir::ExprKind::BoolLiteral(value) => mir::ExprKind::BoolLiteral(*value),
            smir::ExprKind::UnitLiteral => mir::ExprKind::UnitLiteral,
            smir::ExprKind::TupleLiteral(elements) => mir::ExprKind::TupleLiteral(
                elements
                    .iter()
                    .map(|element| self.lower_expr(element, span))
                    .collect(),
            ),
            smir::ExprKind::StructInit { struct_id, args } => mir::ExprKind::StructInit {
                struct_id: *struct_id,
                args: args.iter().map(|arg| self.lower_expr(arg, span)).collect(),
            },
            smir::ExprKind::ClassInit { class_id, args } => mir::ExprKind::ClassInit {
                class_id: *class_id,
                args: args.iter().map(|arg| self.lower_expr(arg, span)).collect(),
            },
            smir::ExprKind::ClosureAlloc { class, captures } => mir::ExprKind::ClosureAlloc {
                class: *class,
                captures: captures
                    .iter()
                    .map(|capture| self.lower_expr(capture, span))
                    .collect(),
            },
            smir::ExprKind::ClosureCapture {
                closure,
                class,
                index,
            } => mir::ExprKind::ClosureCapture {
                closure: Box::new(self.lower_expr(closure, span)),
                class: *class,
                index: *index,
            },
            smir::ExprKind::Local(local) => mir::ExprKind::Local(*local),
            smir::ExprKind::GlobalRead(global) => mir::ExprKind::GlobalRead(*global),
            smir::ExprKind::PtrFromUInt { operand, pointee } => mir::ExprKind::PtrFromUInt {
                operand: Box::new(self.lower_expr(operand, span)),
                pointee: pointee.clone(),
            },
            smir::ExprKind::PtrToUInt(operand) => {
                mir::ExprKind::PtrToUInt(Box::new(self.lower_expr(operand, span)))
            }
            smir::ExprKind::PtrCast { operand, pointee } => mir::ExprKind::PtrCast {
                operand: Box::new(self.lower_expr(operand, span)),
                pointee: pointee.clone(),
            },
            smir::ExprKind::PtrLoad {
                pointer,
                pointee,
                offset,
            } => mir::ExprKind::PtrLoad {
                pointer: Box::new(self.lower_expr(pointer, span)),
                pointee: pointee.clone(),
                offset: offset
                    .as_ref()
                    .map(|offset| Box::new(self.lower_expr(offset, span))),
            },
            smir::ExprKind::PtrStore {
                pointer,
                pointee,
                offset,
                value,
            } => mir::ExprKind::PtrStore {
                pointer: Box::new(self.lower_expr(pointer, span)),
                pointee: pointee.clone(),
                offset: offset
                    .as_ref()
                    .map(|offset| Box::new(self.lower_expr(offset, span))),
                value: Box::new(self.lower_expr(value, span)),
            },
            smir::ExprKind::PtrOffset {
                pointer,
                pointee,
                offset,
                subtract,
            } => mir::ExprKind::PtrOffset {
                pointer: Box::new(self.lower_expr(pointer, span)),
                pointee: pointee.clone(),
                offset: Box::new(self.lower_expr(offset, span)),
                subtract: *subtract,
            },
            smir::ExprKind::AddressOf { local, pointee } => mir::ExprKind::AddressOf {
                local: *local,
                pointee: pointee.clone(),
            },
            smir::ExprKind::GlobalAddress { global, pointee } => mir::ExprKind::GlobalAddress {
                global: *global,
                pointee: pointee.clone(),
            },
            smir::ExprKind::SizeOf(ty) => mir::ExprKind::SizeOf(ty.clone()),
            smir::ExprKind::AlignOf(ty) => mir::ExprKind::AlignOf(ty.clone()),
            smir::ExprKind::FunPtrNull(signature) => mir::ExprKind::FunPtrNull(*signature),
            smir::ExprKind::FunctionAddress { callback } => mir::ExprKind::FunctionAddress {
                callback: *callback,
            },
            smir::ExprKind::ForeignCallbackRegister { bridge, closure } => {
                mir::ExprKind::ForeignCallbackRegister {
                    bridge: *bridge,
                    closure: Box::new(self.lower_expr(closure, span)),
                }
            }
            smir::ExprKind::ForeignCallbackOperation {
                operation,
                callback,
            } => mir::ExprKind::ForeignCallbackOperation {
                operation: *operation,
                callback: Box::new(self.lower_expr(callback, span)),
            },
            smir::ExprKind::Retype { operand, ty } => mir::ExprKind::Retype {
                operand: Box::new(self.lower_expr(operand, span)),
                ty: ty.clone(),
            },
            smir::ExprKind::FieldAccess { receiver, index } => mir::ExprKind::FieldAccess {
                receiver: Box::new(self.lower_expr(receiver, span)),
                index: *index,
            },
            smir::ExprKind::Call(call) => {
                let value = self.lower_call(call, None, span);
                if value.ty == expr.ty {
                    return value;
                }
                return mir::Expr::new(
                    expr.ty.clone(),
                    mir::ExprKind::Retype {
                        operand: Box::new(value),
                        ty: Box::new(expr.ty.clone()),
                    },
                );
            }
            smir::ExprKind::Box(operand) => {
                mir::ExprKind::Box(Box::new(self.lower_expr(operand, span)))
            }
            smir::ExprKind::Unbox(operand) => {
                mir::ExprKind::Unbox(Box::new(self.lower_expr(operand, span)))
            }
            smir::ExprKind::IsInstance { operand, check_ty } => mir::ExprKind::IsInstance {
                operand: Box::new(self.lower_expr(operand, span)),
                check_ty: check_ty.clone(),
            },
            smir::ExprKind::ArrayLiteral {
                array_type,
                elements,
            } => mir::ExprKind::ArrayLiteral {
                array_type: *array_type,
                elements: elements
                    .iter()
                    .map(|element| self.lower_expr(element, span))
                    .collect(),
            },
            smir::ExprKind::ArrayGet {
                array_type,
                array,
                index,
            } => mir::ExprKind::ArrayGet {
                array_type: *array_type,
                array: Box::new(self.lower_expr(array, span)),
                index: Box::new(self.lower_expr(index, span)),
            },
            smir::ExprKind::ArrayLen {
                array_type,
                operand,
            } => mir::ExprKind::ArrayLen {
                array_type: *array_type,
                operand: Box::new(self.lower_expr(operand, span)),
            },
            smir::ExprKind::ArrayClone {
                source_type,
                target_type,
                operand,
            } => mir::ExprKind::ArrayClone {
                source_type: *source_type,
                target_type: *target_type,
                operand: Box::new(self.lower_expr(operand, span)),
            },
            smir::ExprKind::ShortCircuit {
                op: smir::LogicOp::And,
                lhs,
                rhs,
            } => return self.lower_short_circuit(lhs, rhs, false, span),
            smir::ExprKind::ShortCircuit {
                op: smir::LogicOp::Or,
                lhs,
                rhs,
            } => return self.lower_short_circuit(lhs, rhs, true, span),
            smir::ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.lower_expr(lhs, span);
                let rhs = self.lower_expr(rhs, span);
                mir::ExprKind::Binary {
                    op: *op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            smir::ExprKind::Unary { op, operand } => mir::ExprKind::Unary {
                op: *op,
                operand: Box::new(self.lower_expr(operand, span)),
            },
            smir::ExprKind::VariantConstruct { variant, fields } => {
                mir::ExprKind::VariantConstruct {
                    variant: *variant,
                    fields: fields
                        .iter()
                        .map(|field| self.lower_expr(field, span))
                        .collect(),
                }
            }
            smir::ExprKind::EnumTag(operand) => {
                mir::ExprKind::EnumTag(Box::new(self.lower_expr(operand, span)))
            }
            smir::ExprKind::EnumField {
                operand,
                variant,
                index,
            } => mir::ExprKind::EnumField {
                operand: Box::new(self.lower_expr(operand, span)),
                variant: *variant,
                index: *index,
            },
        };
        mir::Expr::new(expr.ty.clone(), kind)
    }

    pub(super) fn lower_short_circuit(
        &mut self,
        lhs: &smir::Expr,
        rhs: &smir::Expr,
        short_value: bool,
        span: Span,
    ) -> mir::Expr {
        let lhs = self.lower_expr(lhs, span);
        let rhs_block = self.new_block("logic.rhs");
        let short_block = self.new_block("logic.short");
        let merge = self.new_block("logic.merge");
        let (then_block, else_block) = if short_value {
            (short_block, rhs_block)
        } else {
            (rhs_block, short_block)
        };
        self.seal(mir::Terminator::Branch {
            cond: lhs,
            then_block,
            else_block,
        });

        let result = self.new_hidden("logic", mir::Type::Boolean);
        self.enter(short_block);
        self.push(
            mir::StatementKind::Assign {
                local: result,
                value: mir::Expr::new(mir::Type::Boolean, mir::ExprKind::BoolLiteral(short_value)),
            },
            span,
        );
        self.seal(mir::Terminator::Goto(merge));

        self.enter(rhs_block);
        let rhs = self.lower_expr(rhs, span);
        self.push(
            mir::StatementKind::Assign {
                local: result,
                value: rhs,
            },
            span,
        );
        self.seal(mir::Terminator::Goto(merge));

        self.enter(merge);
        mir::Expr::new(mir::Type::Boolean, mir::ExprKind::Local(result))
    }

    pub(super) fn lower_call(
        &mut self,
        call: &smir::Call,
        destination: Option<mir::LocalId>,
        span: Span,
    ) -> mir::Expr {
        let args = call
            .args
            .iter()
            .map(|arg| self.lower_expr(arg, span))
            .collect();
        let normalized = mir::Call {
            target: call.target.clone(),
            args,
        };
        if call.return_ty == mir::Type::Unit {
            assert!(
                destination.is_none(),
                "Unit calls do not have MIR destinations"
            );
            self.push(
                mir::StatementKind::Call(mir::CallEffect::Unit(normalized)),
                span,
            );
            mir::Expr::new(mir::Type::Unit, mir::ExprKind::UnitLiteral)
        } else {
            let destination =
                destination.unwrap_or_else(|| self.new_hidden("call", call.return_ty.clone()));
            self.push(
                mir::StatementKind::Call(mir::CallEffect::Value {
                    destination,
                    call: normalized,
                }),
                span,
            );
            mir::Expr::new(call.return_ty.clone(), mir::ExprKind::Local(destination))
        }
    }

    pub(super) fn new_hidden(&mut self, prefix: &str, ty: mir::Type) -> mir::LocalId {
        self.hidden_count += 1;
        self.locals.alloc(mir::Local {
            name: format!("${prefix}.{}", self.hidden_count),
            ty,
            mutable: false,
        })
    }
}
