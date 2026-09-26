use super::*;

impl BodyLowerer<'_> {
    pub(crate) fn lower_expr(&mut self, expr: &hir::Expr) -> smir::Expr {
        if let hir::ExprKind::ConstructorParam(parameter) = expr.kind {
            return self.constructor_param_map[&parameter].clone();
        }
        let ty = self.lower_type(expr.ty);
        if matches!(expr.kind, hir::ExprKind::ConstructorReceiver) {
            let receiver = self
                .constructor_receiver
                .clone()
                .expect("constructor receiver reads only occur in initializer bodies");
            if receiver.ty == ty {
                return receiver;
            }
            return smir::Expr::new(
                ty.clone(),
                smir::ExprKind::Retype {
                    operand: Box::new(receiver),
                    ty: Box::new(ty),
                },
            );
        }
        let kind = match &expr.kind {
            hir::ExprKind::StringLiteral { value, owner } => {
                let id = match *owner {
                    hir::StringConstantOwner::CurrentDefinition => {
                        self.intern_current_string(value.clone())
                    }
                    hir::StringConstantOwner::Property(property) => self.strings.intern(
                        mir::ImmortalObjectOwner::Property(property),
                        0,
                        value.clone(),
                    ),
                };
                smir::ExprKind::StringConst(id)
            }
            hir::ExprKind::IntegerLiteral(value) => {
                smir::ExprKind::IntegerLiteral(lower_integer_constant(*value))
            }
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
            hir::ExprKind::StructConstruct { struct_id, fields } => {
                assert!(
                    matches!(&ty, mir::Type::Struct(found) if *found == self.struct_map[struct_id])
                );
                let expected_fields = self.module.structs[*struct_id]
                    .declared_fields()
                    .iter()
                    .map(|field| field.ty)
                    .collect::<Vec<_>>();
                assert_eq!(fields.len(), expected_fields.len());
                for (field, expected) in fields.iter().zip(expected_fields) {
                    assert_eq!(self.lower_type(field.ty), self.lower_type(expected));
                }
                smir::ExprKind::StructConstruct {
                    struct_id: self.struct_map[struct_id],
                    fields: fields.iter().map(|field| self.lower_expr(field)).collect(),
                }
            }
            hir::ExprKind::StructConstructorCall { constructor, args } => {
                let ctor = self.struct_ctors[constructor];
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(ctor),
                    },
                    args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                    return_ty: ty.clone(),
                })
            }
            hir::ExprKind::ClassNew { constructor, args } => {
                let class = self.module.class_constructors[*constructor].class;
                smir::ExprKind::ClassNew {
                    class_id: self.class_map[&class],
                    initializer: mir::Callee::User(self.ctors[constructor]),
                    args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                }
            }
            hir::ExprKind::ClassInitializerCall {
                receiver,
                initializer,
                args,
            } => {
                let mut lowered = Vec::with_capacity(args.len() + 1);
                lowered.push(self.lower_expr(receiver));
                lowered.extend(args.iter().map(|arg| self.lower_expr(arg)));
                smir::ExprKind::Call(smir::Call {
                    target: mir::CallTarget {
                        kind: mir::CallKind::Direct,
                        callee: mir::Callee::User(self.ctors[initializer]),
                    },
                    args: lowered,
                    return_ty: mir::Type::Unit,
                })
            }
            hir::ExprKind::VariantConstruct { variant, args } => {
                let mir::Type::Enum(enum_id, _) = &ty else {
                    unreachable!("a variant construction has an enum type")
                };
                assert_eq!(self.enums.hir_ids[enum_id], variant.enumeration());
                let expected_fields = self.module.enums[variant.enumeration()].variants
                    [variant.variant().into_raw() as usize]
                    .fields
                    .iter()
                    .map(|field| field.ty)
                    .collect::<Vec<_>>();
                assert_eq!(args.len(), expected_fields.len());
                for (argument, expected) in args.iter().zip(expected_fields) {
                    assert_eq!(self.lower_type(argument.ty), self.lower_type(expected));
                }
                let variant = self.enums.lower_variant_ref(*variant);
                assert_eq!(variant.enum_id(), *enum_id);
                smir::ExprKind::VariantConstruct {
                    variant,
                    fields: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                }
            }
            hir::ExprKind::VariantTest { operand, variant } => {
                assert_eq!(ty, mir::Type::Boolean);
                let operand = self.lower_expr(operand);
                let mir::Type::Enum(enum_id, _) = &operand.ty else {
                    unreachable!("a variant test has an enum operand")
                };
                assert_eq!(self.enums.hir_ids[enum_id], variant.enumeration());
                let variant = self.enums.lower_variant_ref(*variant);
                assert_eq!(variant.enum_id(), *enum_id);
                return smir::Expr::variant_test(&self.enums.defs, operand, variant);
            }
            hir::ExprKind::VariantPayloadProject { operand, field } => {
                let operand = self.lower_expr(operand);
                let mir::Type::Enum(enum_id, _) = &operand.ty else {
                    unreachable!("a variant payload projection has an enum operand")
                };
                let source_variant = field.variant();
                assert_eq!(self.enums.hir_ids[enum_id], source_variant.enumeration());
                let expected = self.module.enums[source_variant.enumeration()].variants
                    [source_variant.variant().into_raw() as usize]
                    .fields[field.local_index() as usize]
                    .ty;
                assert_eq!(ty, self.lower_type(expected));
                let field = self.enums.lower_variant_field_ref(*field);
                assert_eq!(field.variant().enum_id(), *enum_id);
                return smir::Expr::variant_payload_project(&self.enums.defs, operand, field);
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
            hir::ExprKind::ConstructorParam(_) => {
                unreachable!("constructor parameters return before expression lowering")
            }
            hir::ExprKind::ConstructorReceiver => {
                unreachable!("constructor receivers return before expression lowering")
            }
            hir::ExprKind::GlobalRead(global) => {
                smir::ExprKind::GlobalRead(self.global_map[global])
            }
            hir::ExprKind::SingletonValue(value) => {
                let singleton = self.module.singleton_values[*value];
                let unit = &self.module.initialization_units[singleton.initialization];
                self.prelude.push(smir::StatementKind::Expr(smir::Expr::new(
                    mir::Type::Unit,
                    smir::ExprKind::Call(smir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::User(self.function_map[&unit.ensure]),
                        },
                        args: Vec::new(),
                        return_ty: mir::Type::Unit,
                    }),
                )));
                let root = self.singleton_root_map[&singleton.published_root];
                smir::ExprKind::GlobalRead(self.singleton_published_roots[root].global)
            }
            hir::ExprKind::ImportedSingletonValue(value) => {
                let (ensure, global) = self.imported_singleton_map[value];
                self.prelude.push(smir::StatementKind::Expr(smir::Expr::new(
                    mir::Type::Unit,
                    smir::ExprKind::Call(smir::Call {
                        target: mir::CallTarget {
                            kind: mir::CallKind::Direct,
                            callee: mir::Callee::External(ensure),
                        },
                        args: Vec::new(),
                        return_ty: mir::Type::Unit,
                    }),
                )));
                smir::ExprKind::GlobalRead(global)
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
                let sources = self.module.lambdas[*id]
                    .captures
                    .iter()
                    .map(|capture| {
                        (
                            self.closure_capture_indices[&(class, capture.binding)],
                            capture.source.clone(),
                        )
                    })
                    .collect();
                self.lower_closure_allocation(class, sources)
            }
            hir::ExprKind::AnonymousFunction(id) => {
                let class = self.ensure_anonymous_closure(*id);
                let sources = self.module.anonymous_functions[*id]
                    .captures
                    .iter()
                    .map(|capture| {
                        (
                            self.closure_capture_indices[&(class, capture.binding)],
                            capture.source.clone(),
                        )
                    })
                    .collect();
                self.lower_closure_allocation(class, sources)
            }
            hir::ExprKind::CallableReference(id) => {
                let class = self.ensure_reference_closure(*id);
                let reference = &self.module.callable_references[*id];
                let mut sources = Vec::with_capacity(
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
                        sources.push((
                            self.closure_receiver_indices[&class],
                            receiver.as_ref().clone(),
                        ));
                    }
                    _ => {}
                }
                sources.extend(reference.captures.iter().map(|capture| {
                    (
                        self.closure_capture_indices[&(class, capture.binding)],
                        capture.source.clone(),
                    )
                }));
                self.lower_closure_allocation(class, sources)
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
            hir::ExprKind::PtrFromNonZeroULong(operand) => {
                let mir::Type::Ptr(pointee) = self.lower_type(expr.ty) else {
                    unreachable!("PtrFromNonZeroULong has a pointer type")
                };
                let operand = self.lower_expr(operand);
                assert_eq!(
                    operand.ty,
                    mir::Type::Integer(mir::IntegerKind::UNSIGNED_64),
                    "PtrFromNonZeroULong consumes the validated ULong carrier"
                );
                smir::ExprKind::PtrFromNonZeroULong {
                    operand: Box::new(operand),
                    pointee,
                }
            }
            hir::ExprKind::PtrToULong(operand) => {
                smir::ExprKind::PtrToULong(Box::new(self.lower_expr(operand)))
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
            hir::ExprKind::FunctionAddress(function) => {
                let hir::TypeKind::FunPtr(signature) = self.module.types[expr.ty].kind else {
                    unreachable!("FunctionAddress has a FunPtr type")
                };
                let callback = self.ensure_callback_bridge(*function, signature, expr.span);
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
            } => {
                let mir::Type::Struct(callback_type) = self.lower_type(callback.ty) else {
                    unreachable!("validated foreign callback operation has a callback struct")
                };
                let family = self.ensure_foreign_callback_family(callback_type);
                let contract = self.foreign_callback_families[family];
                let operation = match operation {
                    hir::ForeignCallbackOperation::Retain => {
                        assert_eq!(ty, mir::Type::Struct(contract.callback));
                        mir::ForeignCallbackOperation::Retain(family)
                    }
                    hir::ForeignCallbackOperation::Release => {
                        assert_eq!(ty, mir::Type::Unit);
                        mir::ForeignCallbackOperation::Release(family)
                    }
                    hir::ForeignCallbackOperation::State => {
                        assert!(
                            matches!(ty, mir::Type::Enum(id, _) if id == contract.states.enum_id())
                        );
                        mir::ForeignCallbackOperation::State(family)
                    }
                    hir::ForeignCallbackOperation::Failure => {
                        assert!(
                            matches!(ty, mir::Type::Enum(id, _) if id == contract.failure_result.enum_id())
                        );
                        mir::ForeignCallbackOperation::Failure(family)
                    }
                };
                smir::ExprKind::ForeignCallbackOperation {
                    operation,
                    callback: Box::new(self.lower_expr(callback)),
                }
            }
            hir::ExprKind::FieldAccess { receiver, field } => {
                // Struct fields, tuple elements and class constructor
                // properties are all 0-based here (the class index
                // follows the flattened base-prefix layout; LIR turns
                // it into a heap object load).
                let index = match field {
                    hir::FieldRef::StructField(field) => {
                        let receiver_ty = self.lower_type(receiver.ty);
                        assert_eq!(
                            receiver_ty,
                            mir::Type::Struct(self.struct_map[&field.structure()])
                        );
                        let expected = self.module.structs[field.structure()].declared_fields()
                            [field.local_index() as usize]
                            .ty;
                        assert_eq!(ty, self.lower_type(expected));
                        field.local_index()
                    }
                    hir::FieldRef::ClassField { index, .. } | hir::FieldRef::TupleIndex(index) => {
                        *index
                    }
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
            hir::ExprKind::DirectSuperMethodCall {
                receiver,
                callee,
                args,
            } => return self.lower_direct_super_method_call(receiver, *callee, args, expr.ty),
            // `Box` / `Unbox` / `is` stay dedicated MIR nodes; LIR
            // lowers them (the runtime box call, the payload load,
            // the `scoop_rt_is_instance` call). Boxing registers the
            // boxed value type (and the target interface) on the way.
            hir::ExprKind::Box(operand) => {
                let payload = self.lower_type(operand.ty);
                self.register_boxed(
                    &payload,
                    self.module.exact_type_identities[operand.ty].id(),
                    Some(expr.ty),
                );
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
                let exact = *check_ty;
                let check_ty = self.lower_type(exact);
                self.register_check(&check_ty, exact);
                smir::ExprKind::IsInstance {
                    operand: Box::new(self.lower_expr(operand)),
                    check_ty: Box::new(check_ty),
                }
            }
            hir::ExprKind::ReferenceUpcast(operand) => smir::ExprKind::Retype {
                operand: Box::new(self.lower_expr(operand)),
                ty: Box::new(ty.clone()),
            },
            hir::ExprKind::Cast {
                operand,
                check_ty,
                optional,
            } => {
                return self.lower_cast(operand, *check_ty, *optional, expr.ty, expr.span);
            }
            hir::ExprKind::Call { callee, args, .. } => {
                return self.lower_call(*callee, args, expr.ty);
            }

            hir::ExprKind::ImportedDependencyCall { callee, args, .. } => {
                return self.lower_imported_call(*callee, args, expr.ty);
            }
            hir::ExprKind::LocalFunctionCall {
                callee,
                captures,
                args,
                ..
            } => {
                let function = self.module.callable_function(*callee);
                self.record_suspend_function_call(function);
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
                self.contains_suspend_call |= self.module.function_types[*function_type].is_suspend;
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
            hir::ExprKind::PrimitiveBinary { kind, lhs, rhs } => {
                return self.lower_primitive_binary(*kind, lhs, rhs);
            }
            hir::ExprKind::PrimitiveUnary { kind, operand } => {
                return self.lower_primitive_unary(*kind, operand);
            }
            hir::ExprKind::IntegerOperation {
                operation,
                arguments,
            } => return self.lower_integer_operation(operation, arguments, expr.span),
            hir::ExprKind::IntegerConversion {
                conversion,
                operand,
            } => return self.lower_integer_conversion(conversion, operand),
            hir::ExprKind::Binary { op, lhs, rhs } => {
                return self.lower_binary(*op, lhs, rhs);
            }
            hir::ExprKind::Unary { op, operand } => {
                let operand = Box::new(self.lower_expr(operand));
                let op = match op {
                    hir::UnOp::Not => mir::UnOp::BoolNot,
                };
                smir::ExprKind::Unary { op, operand }
            }
            // The Option nodes (hir-lower's `?.` / `?:` / `!!`
            // desugars) become generic construction and checked semantic
            // variant operations on core's `Option` enum (DESIGN 3.3).
            hir::ExprKind::SomeWrap(operand) => {
                let some = option_core_for_type(self.module, self.enums, &ty).some();
                smir::ExprKind::VariantConstruct {
                    variant: some,
                    fields: vec![self.lower_expr(operand)],
                }
            }
            hir::ExprKind::NoneLiteral => {
                let none = option_core_for_type(self.module, self.enums, &ty).none();
                smir::ExprKind::VariantConstruct {
                    variant: none,
                    fields: Vec::new(),
                }
            }
            hir::ExprKind::IsSome(operand) => {
                let option_ty = self.lower_type(operand.ty);
                let some = option_some_refs_for_type(self.module, self.enums, &option_ty);
                let operand = self.lower_stable_option_operand(operand);
                return smir::Expr::variant_test(&self.enums.defs, operand, some.variant);
            }
            hir::ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => {
                let option_ty = self.lower_type(operand.ty);
                let some = option_some_refs_for_type(self.module, self.enums, &option_ty);
                if *trap_on_none {
                    return self.trapping_unwrap(operand, expr.ty, expr.span, some);
                } else {
                    // The surrounding `?.` / `?:` control flow already
                    // guarantees `Some` on this exact stable local.
                    let operand = self.lower_stable_option_operand(operand);
                    let projection = smir::Expr::variant_payload_project(
                        &self.enums.defs,
                        operand,
                        some.payload,
                    );
                    assert_eq!(
                        projection.ty, ty,
                        "the checked Option payload is the HIR unwrap result type"
                    );
                    return projection;
                }
            }
        };
        smir::Expr::new(ty, kind)
    }

    fn lower_closure_allocation(
        &mut self,
        class: mir::ClosureClassId,
        semantic_sources: Vec<(u32, hir::Expr)>,
    ) -> smir::ExprKind {
        let field_count = self.closure_classes[class].captures.len();
        assert_eq!(
            semantic_sources.len(),
            field_count,
            "a closure allocation initializes every physical capture field"
        );
        smir::ExprKind::ClosureAlloc {
            class,
            captures: semantic_sources
                .into_iter()
                .map(|(field, source)| {
                    smir::ClosureCaptureInit::new(field, self.lower_expr(&source))
                })
                .collect(),
        }
    }
}
