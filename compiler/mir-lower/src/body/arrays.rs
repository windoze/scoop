//! Managed, exception-aware initialization of fixed-length arrays.

use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_array_expression(&mut self, expr: &hir::Expr) -> smir::Expr {
        let ty = self.lower_type(expr.ty);
        let kind = match &expr.kind {
            hir::ExprKind::ArrayGenerate { count, initializer } => {
                return self.lower_array_generate(expr.ty, count, initializer, expr.span);
            }
            hir::ExprKind::ArrayLiteral(elements) => {
                let mir::Type::Class(array_type) = self.lower_type(expr.ty) else {
                    unreachable!("an array literal has an intrinsic class type")
                };
                smir::ExprKind::ArrayLiteral {
                    array_type,
                    elements: elements.iter().map(|e| self.lower_expr(e)).collect(),
                }
            }
            hir::ExprKind::ArrayAssembly(assembly) => {
                let mir::Type::Class(array_type) = self.lower_type(expr.ty) else {
                    unreachable!("an array assembly has an intrinsic class type")
                };
                debug_assert_eq!(array_type, self.class_map[&assembly.result_type]);
                smir::ExprKind::ArrayAssembly {
                    array_type,
                    parts: assembly
                        .parts
                        .iter()
                        .map(|part| match part {
                            hir::ArrayAssemblyPart::Element(value) => {
                                smir::ArrayAssemblyPart::Element(self.lower_expr(value))
                            }
                            hir::ArrayAssemblyPart::CopyArray(value) => {
                                smir::ArrayAssemblyPart::CopyArray(self.lower_expr(value))
                            }
                        })
                        .collect(),
                }
            }
            // Subscript read. M8: the bounds check moved here from
            // codegen — the array and the index are evaluated once
            // into hidden locals, then `IndexOutOfBoundsException`
            // throws when the index is out of range (the prelude
            // mechanism `!!` uses).
            hir::ExprKind::Index {
                access,
                receiver,
                index,
            } => {
                debug_assert!(matches!(
                    access,
                    hir::ArrayAccessKind::ImmutableGet | hir::ArrayAccessKind::MutableGet
                ));
                let array_ty = self.lower_type(receiver.ty);
                let mir::Type::Class(array_type) = array_ty else {
                    unreachable!("an array subscript has an intrinsic class receiver")
                };
                let array_slot = self.new_hidden("arr", mir::Type::Class(array_type), false);
                let index_ty = mir::Type::Integer(mir::IntegerKind::SIGNED_64);
                let index_slot = self.new_hidden("idx", index_ty.clone(), false);
                let array = self.lower_expr(receiver);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: array_slot,
                    init: array,
                });
                let index = self.lower_expr(index);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: index_slot,
                    init: index,
                });
                self.bounds_check(array_type, array_slot, index_slot, expr.span);
                smir::ExprKind::ArrayGet {
                    array_type,
                    array: Box::new(smir::Expr::local(array_slot, mir::Type::Class(array_type))),
                    index: Box::new(smir::Expr::local(index_slot, index_ty)),
                }
            }
            hir::ExprKind::ArraySet {
                access,
                receiver,
                index,
                value,
            } => {
                debug_assert_eq!(*access, hir::ArrayAccessKind::MutableSet);
                let array_ty = self.lower_type(receiver.ty);
                let mir::Type::Class(array_type) = array_ty else {
                    unreachable!("an array store has an intrinsic class receiver")
                };
                let array_slot = self.new_hidden("arr", mir::Type::Class(array_type), false);
                let index_ty = mir::Type::Integer(mir::IntegerKind::SIGNED_64);
                let index_slot = self.new_hidden("idx", index_ty.clone(), false);
                let value_ty = self.lower_type(value.ty);
                let value_slot = self.new_hidden("value", value_ty.clone(), false);
                let array = self.lower_expr(receiver);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: array_slot,
                    init: array,
                });
                let index = self.lower_expr(index);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: index_slot,
                    init: index,
                });
                let value = self.lower_expr(value);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: value_slot,
                    init: value,
                });
                self.bounds_check(array_type, array_slot, index_slot, expr.span);
                self.prelude.push(smir::StatementKind::ArraySet {
                    array_type,
                    array: smir::Expr::local(array_slot, mir::Type::Class(array_type)),
                    index: smir::Expr::local(index_slot, index_ty),
                    value: smir::Expr::local(value_slot, value_ty),
                });
                smir::ExprKind::UnitLiteral
            }
            hir::ExprKind::ArrayLen(operand) => {
                let mir::Type::Class(array_type) = self.lower_type(operand.ty) else {
                    unreachable!("array.size has an intrinsic class receiver")
                };
                smir::ExprKind::ArrayLen {
                    array_type,
                    operand: Box::new(self.lower_expr(operand)),
                }
            }
            hir::ExprKind::ArrayClone(operand) => {
                let mir::Type::Class(source_type) = self.lower_type(operand.ty) else {
                    unreachable!("an array conversion has an intrinsic class source")
                };
                let mir::Type::Class(target_type) = self.lower_type(expr.ty) else {
                    unreachable!("an array conversion has an intrinsic class target")
                };
                smir::ExprKind::ArrayClone {
                    source_type,
                    target_type,
                    operand: Box::new(self.lower_expr(operand)),
                }
            }
            _ => unreachable!("the expression dispatcher selects array operations"),
        };
        smir::Expr::new(ty, kind)
    }

    pub(super) fn lower_array_generate(
        &mut self,
        result_type: hir::TypeId,
        count: &hir::Expr,
        initializer: &hir::Expr,
        span: Span,
    ) -> smir::Expr {
        let array_ty = self.lower_type(result_type);
        let mir::Type::Class(array_type) = array_ty else {
            unreachable!("ArrayGenerate has a concrete intrinsic array application")
        };
        let hir::TypeKind::Function(signature) = self.module.types[initializer.ty].kind else {
            unreachable!("ArrayGenerate has an ordinary initializer function")
        };
        let element_type = self.module.function_types[signature].return_type;
        let element_ty = self.lower_type(element_type);
        let function_type = self.lower_function_type_id(signature);
        let closure_ty = self.lower_type(initializer.ty);
        let long_ty = mir::Type::Integer(mir::IntegerKind::SIGNED_64);
        let size = self.new_hidden("array_size", long_ty.clone(), false);
        let closure = self.new_hidden("array_init", closure_ty.clone(), false);
        let array = self.new_hidden("array", array_ty.clone(), false);
        let index = self.new_hidden("array_index", long_ty.clone(), true);
        let value = self.new_hidden("array_value", element_ty.clone(), false);
        let count = self.lower_expr(count);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: size,
            init: count,
        });
        let initializer = self.lower_expr(initializer);
        self.prelude.push(smir::StatementKind::ValDecl {
            local: closure,
            init: initializer,
        });
        let failure = match self.core_protocols {
            hir::ConcreteCoreProtocols::Defined(protocols) => {
                self.throw_builtin(protocols.exceptions.illegal_argument_exception, span)
            }
            hir::ConcreteCoreProtocols::Imported(protocols) => self.throw_imported_exception(
                protocols
                    .exceptions()
                    .illegal_argument_exception()
                    .persistent(),
                protocols
                    .exceptions()
                    .illegal_argument_exception_constructor(),
                span,
            ),
        };
        self.prelude.push(smir::StatementKind::If {
            cond: smir::Expr::integer_compare(
                mir::IntegerComparisonOperation::new(
                    mir::IntegerKind::SIGNED_64,
                    mir::IntegerComparisonOperator::LessThan,
                ),
                smir::Expr::local(size, long_ty.clone()),
                smir::Expr::integer(mir::MirIntegerConstant::Signed64(0)),
            ),
            then_body: vec![failure],
            else_body: None,
        });
        self.prelude.push(smir::StatementKind::ValDecl {
            local: array,
            init: smir::Expr::new(
                array_ty.clone(),
                smir::ExprKind::ArrayAllocate {
                    array_type,
                    count: Box::new(smir::Expr::local(size, long_ty.clone())),
                },
            ),
        });
        self.prelude.push(smir::StatementKind::ValDecl {
            local: index,
            init: smir::Expr::integer(mir::MirIntegerConstant::Signed64(0)),
        });
        let target = smir::LoopId::from_raw(self.next_loop_id);
        self.next_loop_id += 1;
        let statement = |kind| smir::Statement { kind, span };
        self.prelude.push(smir::StatementKind::While {
            target,
            condition_setup: Vec::new(),
            cond: smir::Expr::integer_compare(
                mir::IntegerComparisonOperation::new(
                    mir::IntegerKind::SIGNED_64,
                    mir::IntegerComparisonOperator::LessThan,
                ),
                smir::Expr::local(index, long_ty.clone()),
                smir::Expr::local(size, long_ty.clone()),
            ),
            body: vec![
                statement(smir::StatementKind::ValDecl {
                    local: value,
                    init: smir::Expr::new(
                        element_ty.clone(),
                        smir::ExprKind::Call(smir::Call {
                            target: mir::CallTarget {
                                kind: mir::CallKind::Closure { function_type },
                                callee: mir::Callee::Closure(function_type),
                            },
                            args: vec![
                                smir::Expr::local(closure, closure_ty),
                                smir::Expr::local(index, long_ty.clone()),
                            ],
                            return_ty: element_ty.clone(),
                        }),
                    ),
                }),
                statement(smir::StatementKind::ArraySet {
                    array_type,
                    array: smir::Expr::local(array, array_ty.clone()),
                    index: smir::Expr::local(index, long_ty.clone()),
                    value: smir::Expr::local(value, element_ty),
                }),
                statement(smir::StatementKind::Assign {
                    local: index,
                    value: smir::Expr::integer_binary(
                        mir::IntegerBinaryOperation::new(
                            mir::IntegerKind::SIGNED_64,
                            mir::IntegerBinaryOperator::Add,
                        ),
                        smir::Expr::local(index, long_ty),
                        smir::Expr::integer(mir::MirIntegerConstant::Signed64(1)),
                    ),
                }),
            ],
        });
        smir::Expr::local(array, array_ty)
    }
}
