use super::*;

impl BodyLowerer<'_> {
    pub(crate) fn lower_expr(&mut self, expr: &hir::Expr) -> smir::Expr {
        let ty = self.lower_type(expr.ty);
        let kind = match &expr.kind {
            hir::ExprKind::StringLiteral(value) => {
                // One global constant per literal occurrence, numbered
                // in order of appearance (deterministic).
                let symbol = format!("scoop.str.{}", self.strings.len());
                let id = self.strings.alloc(mir::StringConst {
                    value: value.clone(),
                    symbol,
                });
                smir::ExprKind::StringConst(id)
            }
            hir::ExprKind::IntLiteral(value) => smir::ExprKind::IntLiteral(*value),
            hir::ExprKind::BoolLiteral(value) => smir::ExprKind::BoolLiteral(*value),
            hir::ExprKind::UnitLiteral => smir::ExprKind::UnitLiteral,
            hir::ExprKind::TupleLiteral(elements) => {
                smir::ExprKind::TupleLiteral(elements.iter().map(|e| self.lower_expr(e)).collect())
            }
            hir::ExprKind::StructInit { args, .. } => {
                // The (possibly instantiated) struct def comes from
                // the expression's type: generic applications (M9)
                // resolve to their instance, plain structs to the
                // base definition.
                let mir::Type::Struct(struct_id) = self.lower_type(expr.ty) else {
                    unreachable!("a struct construction has a struct type")
                };
                smir::ExprKind::StructInit {
                    struct_id,
                    args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                }
            }
            // Class construction calls the class's constructor
            // function (`scoop.ctor.<Class>`); the raw allocation and
            // field initialization live inside it (see `lower_ctor`).
            hir::ExprKind::ClassInit { constructor, args } => {
                let ctor = self.ctors[constructor];
                let return_type = self.module.class_constructors[*constructor].return_type;
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(ctor),
                    },
                    args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                    return_ty: self.lower_type(return_type),
                })
            }
            hir::ExprKind::VariantConstruct { variant, args, .. } => {
                smir::ExprKind::VariantConstruct {
                    variant: variant.into_raw(),
                    fields: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                }
            }
            hir::ExprKind::Local(local) => {
                let local = self.local_map[local];
                let narrowed = self.lower_type(expr.ty);
                if self.locals[local].ty == narrowed {
                    smir::ExprKind::Local(local)
                } else if let mir::Type::Function(function_type) = narrowed {
                    return self.adapt_checked_function_value(
                        smir::Expr::local(local, self.locals[local].ty.clone()),
                        function_type,
                        expr.span,
                    );
                } else {
                    smir::ExprKind::Retype {
                        operand: Box::new(smir::Expr::local(local, self.locals[local].ty.clone())),
                        ty: Box::new(narrowed),
                    }
                }
            }
            hir::ExprKind::ConstructorParam(parameter) => {
                smir::ExprKind::Local(self.constructor_param_map[parameter])
            }
            hir::ExprKind::GlobalRead(global) => {
                smir::ExprKind::GlobalRead(self.global_map[global])
            }
            hir::ExprKind::Capture(binding) => {
                if let Some(local) = self.current_local_capture_params.get(binding) {
                    return smir::Expr::new(ty, smir::ExprKind::Local(self.local_map[local]));
                }
                let class = self
                    .current_closure
                    .expect("capture reads only appear in closure invoke bodies");
                let index = self.closure_capture_indices[&(class, *binding)];
                let closure = self
                    .current_closure_local
                    .expect("a closure invoke body has its hidden receiver local");
                smir::ExprKind::ClosureCapture {
                    closure: Box::new(smir::Expr::local(closure, self.locals[closure].ty.clone())),
                    class,
                    index,
                }
            }
            hir::ExprKind::Lambda(id) => {
                let class = self.ensure_lambda_closure(*id);
                let sources: Vec<_> = self.module.lambdas[*id]
                    .captures
                    .iter()
                    .map(|capture| capture.source.clone())
                    .collect();
                smir::ExprKind::ClosureAlloc {
                    class,
                    captures: sources
                        .iter()
                        .map(|source| self.lower_expr(source))
                        .collect(),
                }
            }
            hir::ExprKind::AnonymousFunction(id) => {
                let class = self.ensure_anonymous_closure(*id);
                let sources: Vec<_> = self.module.anonymous_functions[*id]
                    .captures
                    .iter()
                    .map(|capture| capture.source.clone())
                    .collect();
                smir::ExprKind::ClosureAlloc {
                    class,
                    captures: sources
                        .iter()
                        .map(|source| self.lower_expr(source))
                        .collect(),
                }
            }
            hir::ExprKind::CallableReference(id) => {
                let class = self.ensure_reference_closure(*id);
                let reference = &self.module.callable_references[*id];
                let mut captures = Vec::with_capacity(
                    reference.captures.len()
                        + usize::from(matches!(
                            &reference.target,
                            hir::CallableReferenceTarget::BoundMember { .. }
                                | hir::CallableReferenceTarget::BoundExtension { .. }
                        )),
                );
                match &reference.target {
                    hir::CallableReferenceTarget::BoundMember { receiver, .. }
                    | hir::CallableReferenceTarget::BoundExtension { receiver, .. } => {
                        captures.push(self.lower_expr(receiver));
                    }
                    _ => {}
                }
                captures.extend(
                    reference
                        .captures
                        .iter()
                        .map(|capture| self.lower_expr(&capture.source)),
                );
                smir::ExprKind::ClosureAlloc { class, captures }
            }
            hir::ExprKind::FunctionCoercion {
                source,
                coercion,
                target_type,
            } => {
                let conversion = &self.module.function_coercions[*coercion];
                debug_assert_eq!(conversion.target, *target_type);
                let source_type = self.lower_function_type_id(conversion.source);
                let target_type = self.lower_function_type_id(*target_type);
                let value = self.lower_expr(source);
                return self.adapt_function_value(value, source_type, target_type, expr.span);
            }
            // Array nodes carry the exact concrete intrinsic class identity;
            // LIR never reconstructs it from an element layout or context.
            hir::ExprKind::ArrayLiteral(elements) => {
                let mir::Type::Class(array_type) = self.lower_type(expr.ty) else {
                    unreachable!("an array literal has an intrinsic class type")
                };
                smir::ExprKind::ArrayLiteral {
                    array_type,
                    elements: elements.iter().map(|e| self.lower_expr(e)).collect(),
                }
            }
            // Subscript read. M8: the bounds check moved here from
            // codegen — the array and the index are evaluated once
            // into hidden locals, then `IndexOutOfBoundsException`
            // throws when the index is out of range (the prelude
            // mechanism `!!` uses).
            hir::ExprKind::Index { receiver, index } => {
                let array_ty = self.lower_type(receiver.ty);
                let mir::Type::Class(array_type) = array_ty else {
                    unreachable!("an array subscript has an intrinsic class receiver")
                };
                let array_slot = self.new_hidden("arr", mir::Type::Class(array_type), false);
                let index_slot = self.new_hidden("idx", mir::Type::Int, false);
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
                    index: Box::new(smir::Expr::local(index_slot, mir::Type::Int)),
                }
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
            hir::ExprKind::PtrFromUInt(operand) => {
                let mir::Type::Ptr(pointee) = self.lower_type(expr.ty) else {
                    unreachable!("PtrFromUInt has a pointer type")
                };
                smir::ExprKind::PtrFromUInt {
                    operand: Box::new(self.lower_expr(operand)),
                    pointee,
                }
            }
            hir::ExprKind::PtrToUInt(operand) => {
                smir::ExprKind::PtrToUInt(Box::new(self.lower_expr(operand)))
            }
            hir::ExprKind::PtrCast(operand) => {
                let mir::Type::Ptr(pointee) = self.lower_type(expr.ty) else {
                    unreachable!("PtrCast has a pointer type")
                };
                smir::ExprKind::PtrCast {
                    operand: Box::new(self.lower_expr(operand)),
                    pointee,
                }
            }
            hir::ExprKind::PtrLoad { pointer, offset } => {
                let hir::TypeKind::Ptr(pointee) = self.module.types[pointer.ty].kind else {
                    unreachable!("PtrLoad has a pointer operand")
                };
                smir::ExprKind::PtrLoad {
                    pointer: Box::new(self.lower_expr(pointer)),
                    pointee: Box::new(self.lower_type(pointee)),
                    offset: offset
                        .as_ref()
                        .map(|offset| Box::new(self.lower_expr(offset))),
                }
            }
            hir::ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => {
                let hir::TypeKind::Ptr(pointee) = self.module.types[pointer.ty].kind else {
                    unreachable!("PtrStore has a pointer operand")
                };
                smir::ExprKind::PtrStore {
                    pointer: Box::new(self.lower_expr(pointer)),
                    pointee: Box::new(self.lower_type(pointee)),
                    offset: offset
                        .as_ref()
                        .map(|offset| Box::new(self.lower_expr(offset))),
                    value: Box::new(self.lower_expr(value)),
                }
            }
            hir::ExprKind::PtrOffset {
                pointer,
                offset,
                subtract,
            } => {
                let hir::TypeKind::Ptr(pointee) = self.module.types[pointer.ty].kind else {
                    unreachable!("PtrOffset has a pointer operand")
                };
                smir::ExprKind::PtrOffset {
                    pointer: Box::new(self.lower_expr(pointer)),
                    pointee: Box::new(self.lower_type(pointee)),
                    offset: Box::new(self.lower_expr(offset)),
                    subtract: *subtract,
                }
            }
            hir::ExprKind::AddressOf(hir::Place::Local(local)) => {
                let local = self.local_map[local];
                smir::ExprKind::AddressOf {
                    local,
                    pointee: Box::new(self.locals[local].ty.clone()),
                }
            }
            hir::ExprKind::AddressOf(hir::Place::Global(global)) => smir::ExprKind::GlobalAddress {
                global: self.global_map[global],
                pointee: Box::new(self.lower_type(self.module.globals[*global].ty)),
            },
            hir::ExprKind::SizeOf(ty) => smir::ExprKind::SizeOf(Box::new(self.lower_type(*ty))),
            hir::ExprKind::AlignOf(ty) => smir::ExprKind::AlignOf(Box::new(self.lower_type(*ty))),
            hir::ExprKind::FunPtrNull => {
                let mir::Type::FunPtr(signature) = self.lower_type(expr.ty) else {
                    unreachable!("FunPtrNull has a FunPtr type")
                };
                smir::ExprKind::FunPtrNull(signature)
            }
            hir::ExprKind::FunctionAddress(function) => {
                let mir::Type::FunPtr(signature) = self.lower_type(expr.ty) else {
                    unreachable!("FunctionAddress has a FunPtr type")
                };
                let callback =
                    self.ensure_callback_bridge(self.function_map[function], signature, expr.span);
                smir::ExprKind::FunctionAddress { callback }
            }
            hir::ExprKind::ForeignCallbackRegister {
                registration,
                closure,
            } => {
                let bridge = self.ensure_foreign_callback_bridge(*registration, expr.span);
                smir::ExprKind::ForeignCallbackRegister {
                    bridge,
                    closure: Box::new(self.lower_expr(closure)),
                }
            }
            hir::ExprKind::ForeignCallbackOperation {
                operation,
                callback,
            } => smir::ExprKind::ForeignCallbackOperation {
                operation: match operation {
                    hir::ForeignCallbackOperation::Retain => mir::ForeignCallbackOperation::Retain,
                    hir::ForeignCallbackOperation::Release => {
                        mir::ForeignCallbackOperation::Release
                    }
                    hir::ForeignCallbackOperation::State => mir::ForeignCallbackOperation::State,
                    hir::ForeignCallbackOperation::Failure => {
                        mir::ForeignCallbackOperation::Failure
                    }
                },
                callback: Box::new(self.lower_expr(callback)),
            },
            hir::ExprKind::FieldAccess { receiver, field } => {
                // Struct fields, tuple elements and class constructor
                // properties are all 0-based here (the class index
                // follows the flattened base-prefix layout; LIR turns
                // it into a heap object load).
                let index = match field {
                    hir::FieldRef::StructField { index, .. }
                    | hir::FieldRef::ClassField { index, .. }
                    | hir::FieldRef::TupleIndex(index) => *index,
                };
                smir::ExprKind::FieldAccess {
                    receiver: Box::new(self.lower_expr(receiver)),
                    index,
                }
            }
            hir::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => return self.lower_method_call(receiver, *callee, args, expr.ty),
            // `Box` / `Unbox` / `is` stay dedicated MIR nodes; LIR
            // lowers them (the runtime box call, the payload load,
            // the `scoop_rt_is_instance` call). Boxing registers the
            // boxed value type (and the target interface) on the way.
            hir::ExprKind::Box(operand) => {
                let payload = self.lower_type(operand.ty);
                self.register_boxed(&payload, Some(expr.ty));
                smir::ExprKind::Box(Box::new(self.lower_expr(operand)))
            }
            // Smart casts unbox inline wherever the narrowed local is read
            // (e.g. as a field-access receiver). Every unbox is bound to a
            // typed hidden local so the synthetic expression and its result
            // local carry the same complete type.
            hir::ExprKind::Unbox(operand) => {
                let ty = self.lower_type(expr.ty);
                let operand = self.lower_expr(operand);
                let slot = self.new_hidden("ub", ty.clone(), false);
                self.prelude.push(smir::StatementKind::ValDecl {
                    local: slot,
                    init: smir::Expr::new(ty, smir::ExprKind::Unbox(Box::new(operand))),
                });
                smir::ExprKind::Local(slot)
            }
            hir::ExprKind::IsInstance { operand, check_ty } => {
                let check_ty = self.lower_type(*check_ty);
                self.register_check(&check_ty);
                smir::ExprKind::IsInstance {
                    operand: Box::new(self.lower_expr(operand)),
                    check_ty: Box::new(check_ty),
                }
            }
            hir::ExprKind::Cast { operand, optional } => {
                return self.lower_cast(operand, *optional, expr.ty, expr.span);
            }
            hir::ExprKind::Call { callee, args } => {
                return self.lower_call(*callee, args, expr.ty);
            }
            hir::ExprKind::LocalFunctionCall {
                callee,
                captures,
                args,
                ..
            } => {
                let callee = self.lower_user_callee(*callee);
                let call_args: Vec<_> = captures.iter().chain(args).collect();
                let return_ty = self.lower_type(expr.ty);
                return self.call(callee, &call_args, return_ty);
            }
            hir::ExprKind::CallableCall {
                callee,
                function_type,
                args,
            } => {
                let function_type = self.lower_function_type_id(*function_type);
                let mut call_args = Vec::with_capacity(args.len() + 1);
                call_args.push(self.lower_expr(callee));
                call_args.extend(args.iter().map(|arg| self.lower_expr(arg)));
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Closure { function_type },
                        callee: mir::Callee::Closure(function_type),
                    },
                    args: call_args,
                    return_ty: self.lower_type(expr.ty),
                })
            }
            hir::ExprKind::Binary { op, lhs, rhs } => {
                return self.lower_binary(*op, lhs, rhs, expr.span);
            }
            hir::ExprKind::Unary { op, operand } => {
                let operand = Box::new(self.lower_expr(operand));
                let op = match op {
                    hir::UnOp::Neg => mir::UnOp::IntNeg,
                    hir::UnOp::Not => mir::UnOp::BoolNot,
                };
                smir::ExprKind::Unary { op, operand }
            }
            // The Option nodes (hir-lower's `?.` / `?:` / `!!`
            // desugars) become generic enum operations on core's
            // `Option` enum (DESIGN 3.3).
            hir::ExprKind::SomeWrap(operand) => {
                let (some, _) = self.option_variants;
                smir::ExprKind::VariantConstruct {
                    variant: some,
                    fields: vec![self.lower_expr(operand)],
                }
            }
            hir::ExprKind::NoneLiteral => {
                let (_, none) = self.option_variants;
                smir::ExprKind::VariantConstruct {
                    variant: none,
                    fields: Vec::new(),
                }
            }
            hir::ExprKind::IsSome(operand) => {
                let (some, _) = self.option_variants;
                let operand = self.lower_expr(operand);
                smir::ExprKind::Binary {
                    op: mir::BinOp::IntEq,
                    lhs: Box::new(smir::Expr::new(
                        mir::Type::Int,
                        smir::ExprKind::EnumTag(Box::new(operand)),
                    )),
                    rhs: Box::new(smir::Expr::int(i64::from(some))),
                }
            }
            hir::ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => {
                let (some, _) = self.option_variants;
                if *trap_on_none {
                    return self.trapping_unwrap(operand, expr.ty, expr.span, some);
                } else {
                    // The surrounding control flow already guarantees
                    // `Some` (`?.` / `?:` desugars, the equality
                    // expansion).
                    smir::ExprKind::EnumField {
                        operand: Box::new(self.lower_expr(operand)),
                        variant: some,
                        index: 0,
                    }
                }
            }
        };
        smir::Expr::new(ty, kind)
    }
}
