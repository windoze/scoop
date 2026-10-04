use super::*;

impl<'a> CfgLowerer<'a> {
    /// Normalize an expression into a call-free MIR expression, emitting
    /// calls in source evaluation order as explicit effect statements.
    pub(super) fn lower_expr(&mut self, expr: &smir::Expr, span: Span) -> mir::Expr {
        let kind = match &expr.kind {
            smir::ExprKind::StringConst(id) => mir::ExprKind::StringConst(*id),
            smir::ExprKind::IntegerLiteral(value) => mir::ExprKind::IntegerLiteral(*value),
            smir::ExprKind::MachineScalarLiteral(value) => {
                mir::ExprKind::MachineScalarLiteral(*value)
            }
            smir::ExprKind::CharLiteral(value) => mir::ExprKind::CharLiteral(*value),
            smir::ExprKind::CharCode(value) => {
                mir::ExprKind::CharCode(Box::new(self.lower_expr(value, span)))
            }
            smir::ExprKind::CharFromCodeUnchecked(value) => {
                mir::ExprKind::CharFromCodeUnchecked(Box::new(self.lower_expr(value, span)))
            }
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
            smir::ExprKind::StructConstruct { struct_id, fields } => {
                mir::ExprKind::StructConstruct {
                    struct_id: *struct_id,
                    fields: fields
                        .iter()
                        .map(|field| self.lower_expr(field, span))
                        .collect(),
                }
            }
            smir::ExprKind::ClassNew {
                class_id,
                publish_release,
                initializer,
                args,
            } => {
                let mut args = args
                    .iter()
                    .map(|arg| self.lower_expr(arg, span))
                    .collect::<Vec<_>>();
                let receiver_ty = mir::Type::Class(*class_id);
                let receiver = self.new_hidden(
                    "new",
                    StructuralDefinitionSiteRole::SyntheticValue,
                    SyntheticLocalRole::Temporary,
                    receiver_ty.clone(),
                );
                self.push(
                    mir::StatementKind::Assign {
                        local: receiver,
                        value: mir::Expr::new(
                            receiver_ty.clone(),
                            mir::ExprKind::ClassAlloc {
                                class_id: *class_id,
                            },
                        ),
                    },
                    span,
                );
                args.insert(
                    0,
                    mir::Expr::new(receiver_ty.clone(), mir::ExprKind::Local(receiver)),
                );
                self.emit_lowered_call(
                    mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: *initializer,
                    },
                    args,
                    mir::Type::Unit,
                    None,
                    span,
                );
                if *publish_release {
                    self.push(
                        mir::StatementKind::PublishReleaseReady {
                            class: *class_id,
                            receiver: mir::Expr::new(
                                receiver_ty.clone(),
                                mir::ExprKind::Local(receiver),
                            ),
                        },
                        span,
                    );
                }
                return mir::Expr::new(receiver_ty, mir::ExprKind::Local(receiver));
            }
            smir::ExprKind::ClosureAlloc { class, captures } => mir::ExprKind::ClosureAlloc {
                class: *class,
                captures: captures
                    .iter()
                    .map(|capture| {
                        mir::ClosureCaptureInit::new(
                            capture.field,
                            self.lower_expr(&capture.value, span),
                        )
                    })
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
            smir::ExprKind::InitializationUnitAddress(unit) => {
                mir::ExprKind::InitializationUnitAddress(*unit)
            }
            smir::ExprKind::PtrFromNonZeroULong { operand, pointee } => {
                return mir::Expr::ptr_from_non_zero_ulong(
                    self.lower_expr(operand, span),
                    (**pointee).clone(),
                );
            }
            smir::ExprKind::PtrToULong(operand) => {
                mir::ExprKind::PtrToULong(Box::new(self.lower_expr(operand, span)))
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
            smir::ExprKind::ReleaseFieldLoad { class, index } => mir::ExprKind::ReleaseFieldLoad {
                class: *class,
                index: *index,
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
            smir::ExprKind::ArrayAllocate { array_type, count } => mir::ExprKind::ArrayAllocate {
                array_type: *array_type,
                count: Box::new(self.lower_expr(count, span)),
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
            smir::ExprKind::ArrayAssembly { array_type, parts } => mir::ExprKind::ArrayAssembly {
                array_type: *array_type,
                parts: parts
                    .iter()
                    .map(|part| match part {
                        smir::ArrayAssemblyPart::Element(value) => {
                            mir::ArrayAssemblyPart::Element(self.lower_expr(value, span))
                        }
                        smir::ArrayAssemblyPart::CopyArray(value) => {
                            mir::ArrayAssemblyPart::CopyArray(self.lower_expr(value, span))
                        }
                    })
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
            smir::ExprKind::IntegerUnary { operation, operand } => mir::ExprKind::IntegerUnary {
                operation: *operation,
                operand: Box::new(self.lower_expr(operand, span)),
            },
            smir::ExprKind::IntegerBinary {
                operation,
                lhs,
                rhs,
            } => mir::ExprKind::IntegerBinary {
                operation: *operation,
                lhs: Box::new(self.lower_expr(lhs, span)),
                rhs: Box::new(self.lower_expr(rhs, span)),
            },
            smir::ExprKind::SafeIntegerDivRem {
                operation,
                lhs,
                rhs,
            } => mir::ExprKind::SafeIntegerDivRem {
                operation: *operation,
                lhs: Box::new(self.lower_expr(lhs, span)),
                rhs: Box::new(self.lower_expr(rhs, span)),
            },
            smir::ExprKind::IntegerCompare {
                operation,
                lhs,
                rhs,
            } => mir::ExprKind::IntegerCompare {
                operation: *operation,
                lhs: Box::new(self.lower_expr(lhs, span)),
                rhs: Box::new(self.lower_expr(rhs, span)),
            },
            smir::ExprKind::IntegerCompareTo {
                operation,
                lhs,
                rhs,
            } => mir::ExprKind::IntegerCompareTo {
                operation: *operation,
                lhs: Box::new(self.lower_expr(lhs, span)),
                rhs: Box::new(self.lower_expr(rhs, span)),
            },
            smir::ExprKind::IntegerShift {
                operation,
                value,
                count,
            } => mir::ExprKind::IntegerShift {
                operation: *operation,
                value: Box::new(self.lower_expr(value, span)),
                count: Box::new(self.lower_expr(count, span)),
            },
            smir::ExprKind::IntegerConversion {
                conversion,
                operand,
            } => mir::ExprKind::IntegerConversion {
                conversion: *conversion,
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
            smir::ExprKind::VariantTest { operand, variant } => {
                let operand = self.lower_expr(operand, span);
                let lowered = mir::Expr::variant_test(self.enums, operand, *variant)
                    .expect("structured MIR preserves its checked variant-test contract");
                assert_eq!(lowered.ty, expr.ty);
                return lowered;
            }
            smir::ExprKind::VariantPayloadProject { operand, field } => {
                let operand = self.lower_expr(operand, span);
                let lowered = mir::Expr::variant_payload_project(self.enums, operand, *field)
                    .expect("structured MIR preserves its checked payload-projection contract");
                assert_eq!(lowered.ty, expr.ty);
                return lowered;
            }
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

        let result = self.new_hidden(
            "logic",
            StructuralDefinitionSiteRole::SyntheticValue,
            SyntheticLocalRole::Temporary,
            mir::Type::Boolean,
        );
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
        self.emit_lowered_call(
            call.target.clone(),
            args,
            call.return_ty.clone(),
            destination,
            span,
        )
    }

    fn emit_lowered_call(
        &mut self,
        target: mir::CallTarget,
        args: Vec<mir::Expr>,
        return_ty: mir::Type,
        destination: Option<mir::LocalId>,
        span: Span,
    ) -> mir::Expr {
        let normalized = mir::Call {
            target,
            args,
            pending: self.call_pending_context(),
        };
        if return_ty == mir::Type::Unit {
            assert!(
                destination.is_none(),
                "Unit calls do not have MIR destinations"
            );
            self.push_call(mir::CallEffect::Unit(normalized), span);
            mir::Expr::new(mir::Type::Unit, mir::ExprKind::UnitLiteral)
        } else {
            let destination = destination.unwrap_or_else(|| {
                self.new_hidden(
                    "call",
                    StructuralDefinitionSiteRole::SyntheticValue,
                    SyntheticLocalRole::Temporary,
                    return_ty.clone(),
                )
            });
            self.push_call(
                mir::CallEffect::Value {
                    destination,
                    call: normalized,
                },
                span,
            );
            mir::Expr::new(return_ty, mir::ExprKind::Local(destination))
        }
    }

    fn push_call(&mut self, effect: mir::CallEffect, span: Span) {
        self.ensure_unwind_context();
        self.call_sites.push(CallSite {
            block: self.current,
            statement: self.blocks[self.current].statements.len(),
        });
        self.push(mir::StatementKind::Call(effect), span);
    }

    pub(super) fn new_hidden(
        &mut self,
        prefix: &str,
        site_role: StructuralDefinitionSiteRole,
        role: SyntheticLocalRole,
        ty: mir::Type,
    ) -> mir::LocalId {
        self.hidden_count += 1;
        let local = self.locals.alloc(mir::Local {
            name: format!("${prefix}.{}", self.hidden_count),
            ty,
            mutable: false,
        });
        self.generated_values.push(GeneratedLocalValue {
            local,
            site_role,
            role,
        });
        local
    }
}
