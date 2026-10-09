use super::*;

impl<'a> CfgLowerer<'a> {
    /// Normalize an expression into a call-free MIR expression, emitting
    /// calls in source evaluation order as explicit effect statements.
    pub(super) fn lower_expr(&mut self, expr: &smir::Expr, span: Span) -> Option<mir::Expr> {
        let kind = match &expr.kind {
            smir::ExprKind::Diverging { prefix, terminal } => {
                for value in prefix {
                    let value = self.lower_expr(value, span)?;
                    self.push(mir::StatementKind::Expr(value), span);
                }
                return self.lower_expr(terminal, span);
            }
            smir::ExprKind::MaybeUninit { wrapper, operation } => {
                let operation = match operation {
                    mir::MaybeUninitOperation::Uninit => mir::MaybeUninitOperation::Uninit,
                    mir::MaybeUninitOperation::Initialized(value) => {
                        mir::MaybeUninitOperation::Initialized(Box::new(
                            self.lower_expr(value, span)?,
                        ))
                    }
                    mir::MaybeUninitOperation::AssumeInit(value) => {
                        mir::MaybeUninitOperation::AssumeInit(Box::new(
                            self.lower_expr(value, span)?,
                        ))
                    }
                };
                mir::ExprKind::MaybeUninit {
                    wrapper: *wrapper,
                    operation,
                }
            }
            smir::ExprKind::DataBorrow(operation) => {
                mir::ExprKind::DataBorrow(mir::DataBorrowOperation {
                    kind: operation.kind,
                    operand: Box::new(self.lower_expr(&operation.operand, span)?),
                })
            }
            smir::ExprKind::Context(operation) => {
                let mut value = match operation.operand() {
                    Some(operand) => Some(self.lower_expr(operand, span)?),
                    None => None,
                };
                mir::ExprKind::Context(operation.map(|_| {
                    value
                        .take()
                        .expect("a context operation has at most one operand")
                }))
            }
            smir::ExprKind::StringConst(id) => mir::ExprKind::StringConst(*id),
            smir::ExprKind::IntegerLiteral(value) => mir::ExprKind::IntegerLiteral(*value),
            smir::ExprKind::MachineScalarLiteral(value) => {
                mir::ExprKind::MachineScalarLiteral(*value)
            }
            smir::ExprKind::FloatLiteral(value) => mir::ExprKind::FloatLiteral(*value),
            smir::ExprKind::FloatUnary {
                kind,
                operation,
                operand,
            } => mir::ExprKind::FloatUnary {
                kind: *kind,
                operation: *operation,
                operand: Box::new(self.lower_expr(operand, span)?),
            },
            smir::ExprKind::FloatBinary {
                kind,
                operation,
                lhs,
                rhs,
            } => {
                let (lhs, rhs) = self.lower_pair(lhs, rhs, span)?;
                mir::ExprKind::FloatBinary {
                    kind: *kind,
                    operation: *operation,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            smir::ExprKind::FloatConversion {
                conversion,
                operand,
            } => mir::ExprKind::FloatConversion {
                conversion: *conversion,
                operand: Box::new(self.lower_expr(operand, span)?),
            },
            smir::ExprKind::CharLiteral(value) => mir::ExprKind::CharLiteral(*value),
            smir::ExprKind::CharCode(value) => {
                mir::ExprKind::CharCode(Box::new(self.lower_expr(value, span)?))
            }
            smir::ExprKind::CharFromCodeUnchecked(value) => {
                mir::ExprKind::CharFromCodeUnchecked(Box::new(self.lower_expr(value, span)?))
            }
            smir::ExprKind::BoolLiteral(value) => mir::ExprKind::BoolLiteral(*value),
            smir::ExprKind::UnitLiteral => mir::ExprKind::UnitLiteral,
            smir::ExprKind::TupleLiteral(elements) => {
                mir::ExprKind::TupleLiteral(self.lower_operands(elements.iter(), span)?)
            }
            smir::ExprKind::StructInit { struct_id, args } => mir::ExprKind::StructInit {
                struct_id: *struct_id,
                args: self.lower_operands(args.iter(), span)?,
            },
            smir::ExprKind::StructConstruct { struct_id, fields } => {
                mir::ExprKind::StructConstruct {
                    struct_id: *struct_id,
                    fields: self.lower_operands(fields.iter(), span)?,
                }
            }
            smir::ExprKind::ClassNew {
                class_id,
                publish_release,
                initializer,
                args,
            } => {
                let mut args = self.lower_operands(args.iter(), span)?;
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
                )?;
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
                return Some(mir::Expr::new(receiver_ty, mir::ExprKind::Local(receiver)));
            }
            smir::ExprKind::ClosureAlloc { class, captures } => {
                let values =
                    self.lower_operands(captures.iter().map(|capture| &capture.value), span)?;
                mir::ExprKind::ClosureAlloc {
                    class: *class,
                    captures: captures
                        .iter()
                        .zip(values)
                        .map(|(capture, value)| mir::ClosureCaptureInit::new(capture.field, value))
                        .collect(),
                }
            }
            smir::ExprKind::ClosureCapture {
                closure,
                class,
                index,
            } => mir::ExprKind::ClosureCapture {
                closure: Box::new(self.lower_expr(closure, span)?),
                class: *class,
                index: *index,
            },
            smir::ExprKind::Local(local) => mir::ExprKind::Local(*local),
            smir::ExprKind::GlobalRead(global) => mir::ExprKind::GlobalRead(*global),
            smir::ExprKind::InitializationUnitAddress(unit) => {
                mir::ExprKind::InitializationUnitAddress(*unit)
            }
            smir::ExprKind::PtrFromNonZeroULong { operand, pointee } => {
                return Some(mir::Expr::ptr_from_non_zero_ulong(
                    self.lower_expr(operand, span)?,
                    (**pointee).clone(),
                ));
            }
            smir::ExprKind::PtrToULong(operand) => {
                mir::ExprKind::PtrToULong(Box::new(self.lower_expr(operand, span)?))
            }
            smir::ExprKind::PtrCast { operand, pointee } => mir::ExprKind::PtrCast {
                operand: Box::new(self.lower_expr(operand, span)?),
                pointee: pointee.clone(),
            },
            smir::ExprKind::PtrLoad {
                pointer,
                pointee,
                offset,
            } => {
                let (pointer, offset) = match offset {
                    Some(offset) => {
                        let (pointer, offset) = self.lower_pair(pointer, offset, span)?;
                        (pointer, Some(Box::new(offset)))
                    }
                    None => (self.lower_expr(pointer, span)?, None),
                };
                mir::ExprKind::PtrLoad {
                    pointer: Box::new(pointer),
                    pointee: pointee.clone(),
                    offset,
                }
            }
            smir::ExprKind::PtrStore {
                pointer,
                pointee,
                offset,
                value,
            } => {
                let mut values = self
                    .lower_operands(
                        std::iter::once(pointer.as_ref())
                            .chain(offset.as_deref())
                            .chain(std::iter::once(value.as_ref())),
                        span,
                    )?
                    .into_iter();
                mir::ExprKind::PtrStore {
                    pointer: Box::new(values.next().expect("pointer operand")),
                    pointee: pointee.clone(),
                    offset: offset
                        .as_ref()
                        .map(|_| Box::new(values.next().expect("offset operand"))),
                    value: Box::new(values.next().expect("stored value operand")),
                }
            }
            smir::ExprKind::PtrOffset {
                pointer,
                pointee,
                offset,
                subtract,
            } => {
                let (pointer, offset) = self.lower_pair(pointer, offset, span)?;
                mir::ExprKind::PtrOffset {
                    pointer: Box::new(pointer),
                    pointee: pointee.clone(),
                    offset: Box::new(offset),
                    subtract: *subtract,
                }
            }
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
                    closure: Box::new(self.lower_expr(closure, span)?),
                }
            }
            smir::ExprKind::ForeignCallbackOperation {
                operation,
                callback,
            } => mir::ExprKind::ForeignCallbackOperation {
                operation: *operation,
                callback: Box::new(self.lower_expr(callback, span)?),
            },
            smir::ExprKind::Retype { operand, ty } => mir::ExprKind::Retype {
                operand: Box::new(self.lower_expr(operand, span)?),
                ty: ty.clone(),
            },
            smir::ExprKind::ReleaseFieldLoad { class, index } => mir::ExprKind::ReleaseFieldLoad {
                class: *class,
                index: *index,
            },
            smir::ExprKind::FieldAccess { receiver, index } => mir::ExprKind::FieldAccess {
                receiver: Box::new(self.lower_expr(receiver, span)?),
                index: *index,
            },
            smir::ExprKind::Call(call) => {
                let value = self.lower_call(call, None, span)?;
                if value.ty == expr.ty {
                    return Some(value);
                }
                return Some(mir::Expr::new(
                    expr.ty.clone(),
                    mir::ExprKind::Retype {
                        operand: Box::new(value),
                        ty: Box::new(expr.ty.clone()),
                    },
                ));
            }
            smir::ExprKind::Box(operand) => {
                mir::ExprKind::Box(Box::new(self.lower_expr(operand, span)?))
            }
            smir::ExprKind::Unbox(operand) => {
                mir::ExprKind::Unbox(Box::new(self.lower_expr(operand, span)?))
            }
            smir::ExprKind::IsInstance { operand, check_ty } => mir::ExprKind::IsInstance {
                operand: Box::new(self.lower_expr(operand, span)?),
                check_ty: check_ty.clone(),
            },
            smir::ExprKind::ArrayAllocate { array_type, count } => mir::ExprKind::ArrayAllocate {
                array_type: *array_type,
                count: Box::new(self.lower_expr(count, span)?),
            },
            smir::ExprKind::ArrayLiteral {
                array_type,
                elements,
            } => mir::ExprKind::ArrayLiteral {
                array_type: *array_type,
                elements: self.lower_operands(elements.iter(), span)?,
            },
            smir::ExprKind::ArrayAssembly { array_type, parts } => {
                let values = self.lower_operands(
                    parts.iter().map(|part| match part {
                        smir::ArrayAssemblyPart::Element(value)
                        | smir::ArrayAssemblyPart::CopyArray(value) => value,
                    }),
                    span,
                )?;
                mir::ExprKind::ArrayAssembly {
                    array_type: *array_type,
                    parts: parts
                        .iter()
                        .zip(values)
                        .map(|(part, value)| match part {
                            smir::ArrayAssemblyPart::Element(_) => {
                                mir::ArrayAssemblyPart::Element(value)
                            }
                            smir::ArrayAssemblyPart::CopyArray(_) => {
                                mir::ArrayAssemblyPart::CopyArray(value)
                            }
                        })
                        .collect(),
                }
            }
            smir::ExprKind::ArrayGet {
                array_type,
                array,
                index,
            } => {
                let (array, index) = self.lower_pair(array, index, span)?;
                mir::ExprKind::ArrayGet {
                    array_type: *array_type,
                    array: Box::new(array),
                    index: Box::new(index),
                }
            }
            smir::ExprKind::ArrayLen {
                array_type,
                operand,
            } => mir::ExprKind::ArrayLen {
                array_type: *array_type,
                operand: Box::new(self.lower_expr(operand, span)?),
            },
            smir::ExprKind::ArrayClone {
                source_type,
                target_type,
                operand,
            } => mir::ExprKind::ArrayClone {
                source_type: *source_type,
                target_type: *target_type,
                operand: Box::new(self.lower_expr(operand, span)?),
            },
            smir::ExprKind::AtomicNew(initial) => {
                mir::ExprKind::AtomicNew(Box::new(self.lower_expr(initial, span)?))
            }
            smir::ExprKind::Atomic(atomic) => {
                let mut values = self.lower_operands(atomic.operands(), span)?.into_iter();
                mir::ExprKind::Atomic(Box::new(
                    atomic.map(|_| values.next().expect("atomic operand")),
                ))
            }
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
                let (lhs, rhs) = self.lower_pair(lhs, rhs, span)?;
                mir::ExprKind::Binary {
                    op: *op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            smir::ExprKind::Unary { op, operand } => mir::ExprKind::Unary {
                op: *op,
                operand: Box::new(self.lower_expr(operand, span)?),
            },
            smir::ExprKind::IntegerUnary { operation, operand } => mir::ExprKind::IntegerUnary {
                operation: *operation,
                operand: Box::new(self.lower_expr(operand, span)?),
            },
            smir::ExprKind::IntegerBinary {
                operation,
                lhs,
                rhs,
            } => {
                let (lhs, rhs) = self.lower_pair(lhs, rhs, span)?;
                mir::ExprKind::IntegerBinary {
                    operation: *operation,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            smir::ExprKind::SafeIntegerDivRem {
                operation,
                lhs,
                rhs,
            } => {
                let (lhs, rhs) = self.lower_pair(lhs, rhs, span)?;
                mir::ExprKind::SafeIntegerDivRem {
                    operation: *operation,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            smir::ExprKind::IntegerCompare {
                operation,
                lhs,
                rhs,
            } => {
                let (lhs, rhs) = self.lower_pair(lhs, rhs, span)?;
                mir::ExprKind::IntegerCompare {
                    operation: *operation,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            smir::ExprKind::IntegerCompareTo {
                operation,
                lhs,
                rhs,
            } => {
                let (lhs, rhs) = self.lower_pair(lhs, rhs, span)?;
                mir::ExprKind::IntegerCompareTo {
                    operation: *operation,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            smir::ExprKind::IntegerShift {
                operation,
                value,
                count,
            } => {
                let (value, count) = self.lower_pair(value, count, span)?;
                mir::ExprKind::IntegerShift {
                    operation: *operation,
                    value: Box::new(value),
                    count: Box::new(count),
                }
            }
            smir::ExprKind::IntegerConversion {
                conversion,
                operand,
            } => mir::ExprKind::IntegerConversion {
                conversion: *conversion,
                operand: Box::new(self.lower_expr(operand, span)?),
            },
            smir::ExprKind::VariantConstruct { variant, fields } => {
                mir::ExprKind::VariantConstruct {
                    variant: *variant,
                    fields: self.lower_operands(fields.iter(), span)?,
                }
            }
            smir::ExprKind::VariantTest { operand, variant } => {
                let operand = self.lower_expr(operand, span)?;
                let lowered = mir::Expr::variant_test(self.enums, operand, *variant)
                    .expect("structured MIR preserves its checked variant-test contract");
                assert_eq!(lowered.ty, expr.ty);
                return Some(lowered);
            }
            smir::ExprKind::VariantPayloadProject { operand, field } => {
                let operand = self.lower_expr(operand, span)?;
                let lowered = mir::Expr::variant_payload_project(self.enums, operand, *field)
                    .expect("structured MIR preserves its checked payload-projection contract");
                assert_eq!(lowered.ty, expr.ty);
                return Some(lowered);
            }
        };
        let value = mir::Expr::new(expr.ty.clone(), kind);
        if self.is_nothing(&value.ty) {
            self.push(mir::StatementKind::Expr(value), span);
            self.seal(mir::Terminator::Unreachable);
            None
        } else {
            Some(value)
        }
    }
}
