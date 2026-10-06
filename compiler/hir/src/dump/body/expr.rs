use super::*;

pub(super) fn dump_expr(
    module: &Module,
    locals: &Arena<Local>,
    expr: &Expr,
    indent: usize,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    let ty = type_name(module, expr.ty);
    match &expr.kind {
        ExprKind::ContextLookup(requirement) => out.push_str(&format!(
            "{pad}ContextLookup {} #{} : {ty}\n",
            requirement.diagnostic.declaration,
            requirement.parameter.0 + 1,
        )),
        ExprKind::ReleaseFieldLoad(field) => out.push_str(&format!(
            "{pad}ReleaseFieldLoad {}.{} : {ty}\n",
            type_name(module, field.owner),
            field.field,
        )),
        ExprKind::GenericDelegateStorageRead(reference) => out.push_str(&format!(
            "{pad}DelegateStorageRead {} : {ty}\n",
            generic_delegate_name(module, reference)
        )),
        ExprKind::StringLiteral { value, .. } => {
            out.push_str(&format!("{pad}StringLiteral {value:?} : {ty}\n"));
        }
        ExprKind::IntegerLiteral(value) => {
            out.push_str(&format!("{pad}IntegerLiteral {value} : {ty}\n"));
        }
        ExprKind::FloatLiteral(value) => {
            out.push_str(&format!("{pad}FloatLiteral {value} : {ty}\n"))
        }
        ExprKind::CharLiteral(value) => {
            out.push_str(&format!("{pad}CharLiteral {value:?} : {ty}\n"))
        }
        ExprKind::CharCode(operand) => {
            out.push_str(&format!("{pad}CharCode\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::CharFromCodeUnchecked(operand) => {
            out.push_str(&format!("{pad}CharFromCodeUnchecked\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::BoolLiteral(value) => out.push_str(&format!("{pad}BoolLiteral {value} : {ty}\n")),
        ExprKind::UnitLiteral => out.push_str(&format!("{pad}UnitLiteral : {ty}\n")),
        ExprKind::TupleLiteral(elements) => {
            out.push_str(&format!("{pad}TupleLiteral : {ty}\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        ExprKind::ClassInit { constructor, args } => {
            let constructor = &module.class_constructor_applications[*constructor];
            let application = &module.class_applications[constructor.owner];
            let constructed = type_name(module, application.canonical_type);
            out.push_str(&format!("{pad}ClassInit {constructed} : {ty}\n"));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::ConstructorParam(parameter) => out.push_str(&format!(
            "{pad}ConstructorParam #{} : {ty}\n",
            parameter.into_raw()
        )),
        ExprKind::StructInit { constructor, args } => {
            let constructor = &module.struct_constructor_applications[*constructor];
            out.push_str(&format!(
                "{pad}StructInit {} : {ty}\n",
                module.struct_name(module.struct_applications[constructor.owner].template)
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::StructConstruct {
            application,
            fields,
        } => {
            let application = &module.struct_applications[*application];
            out.push_str(&format!(
                "{pad}StructConstruct {} : {ty}\n",
                type_name(module, application.canonical_type)
            ));
            for field in fields {
                dump_expr(module, locals, field, indent + 1, out);
            }
        }
        ExprKind::VariantConstruct { variant, args } => {
            let (name, variant_name, arguments, _) = enum_variant_names(module, *variant);
            let arguments = if arguments.is_empty() {
                String::new()
            } else {
                format!(
                    "<{}>",
                    arguments
                        .iter()
                        .map(|ty| type_name(module, *ty))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            out.push_str(&format!(
                "{pad}VariantConstruct {name}.{variant_name}{arguments} : {ty}\n"
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::VariantTest { operand, variant } => {
            let (_, name, _, _) = enum_variant_names(module, *variant);
            out.push_str(&format!(
                "{pad}VariantTest {}.{name} : {ty}\n",
                type_name(module, variant.owner)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::VariantPayloadProject { operand, field } => {
            let (_, name, _, _) = enum_variant_names(module, field.variant);
            out.push_str(&format!(
                "{pad}VariantPayloadProject {}.{name}.{} : {ty}\n",
                type_name(module, field.variant.owner),
                enum_field_name(module, *field)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Local(local) => {
            out.push_str(&format!("{pad}Local {} : {ty}\n", locals[*local].name));
        }
        ExprKind::GlobalRead(global) => out.push_str(&format!(
            "{pad}GlobalRead {} : {ty}\n",
            module.globals[*global].name
        )),
        ExprKind::SingletonValue(value) => {
            let name = match *value {
                SingletonValueTarget::Local(value) => {
                    let declaration = module.singleton_values[value].declaration;
                    module.objects[declaration].name.clone()
                }
                SingletonValueTarget::Dependency(value) => format!("external {value}"),
            };
            out.push_str(&format!("{pad}SingletonValue {name} : {ty}\n"));
        }
        ExprKind::Capture(binding) => {
            out.push_str(&format!(
                "{pad}Capture binding{} : {ty}\n",
                binding.into_raw()
            ));
        }
        ExprKind::Lambda(id) => {
            let lambda = &module.lambdas[*id];
            out.push_str(&format!(
                "{pad}Lambda lambda{} invoke={} captures={} : {ty}\n",
                id.into_raw(),
                lexical_function_name(module, lambda.definition),
                lambda.captures.len()
            ));
            for capture in &lambda.captures {
                out.push_str(&format!(
                    "{}capture {} binding{} : {}\n",
                    "  ".repeat(indent + 1),
                    capture.name,
                    capture.binding.into_raw(),
                    type_name(module, capture.ty)
                ));
            }
        }
        ExprKind::AnonymousFunction(id) => {
            let anonymous = &module.anonymous_functions[*id];
            out.push_str(&format!(
                "{pad}AnonymousFunction anonymous{} invoke={} captures={} : {ty}\n",
                id.into_raw(),
                lexical_function_name(module, anonymous.definition),
                anonymous.captures.len()
            ));
            for capture in &anonymous.captures {
                out.push_str(&format!(
                    "{}capture {} binding{} : {}\n",
                    "  ".repeat(indent + 1),
                    capture.name,
                    capture.binding.into_raw(),
                    type_name(module, capture.ty)
                ));
            }
        }
        ExprKind::CallableReference(id) => {
            let reference = &module.callable_references[*id];
            let function = match &reference.target {
                CallableReferenceTarget::Named(callee)
                | CallableReferenceTarget::Local { callee, .. }
                | CallableReferenceTarget::BoundExtension { callee, .. } => {
                    method_callee_name(module, MethodCallee::Callable(*callee))
                }
                CallableReferenceTarget::BoundMember { callee, .. } => {
                    method_callee_name(module, *callee)
                }
                CallableReferenceTarget::BoundIntrinsic { intrinsic, .. } => {
                    format!("intrinsic {intrinsic:?}")
                }
            };
            out.push_str(&format!(
                "{pad}CallableReference reference{} target={} captures={} : {ty}\n",
                id.into_raw(),
                function,
                reference.captures.len()
            ));
            if let Some(receiver) = reference.target.receiver() {
                dump_expr(module, locals, receiver, indent + 1, out);
            }
        }
        ExprKind::FunctionCoercion {
            source,
            coercion,
            target_type,
        } => {
            let conversion = &module.function_coercions[*coercion];
            debug_assert_eq!(conversion.target, *target_type);
            out.push_str(&format!(
                "{pad}FunctionCoercion coercion{} {} -> {} : {ty}\n",
                coercion.into_raw().into_u32(),
                type_name(
                    module,
                    module
                        .types
                        .iter()
                        .find_map(|(ty, value)| {
                            matches!(value, Type::Function(id) if *id == conversion.source)
                                .then_some(ty)
                        })
                        .expect("a function signature has a canonical type")
                ),
                type_name(
                    module,
                    module
                        .types
                        .iter()
                        .find_map(|(ty, value)| {
                            matches!(value, Type::Function(id) if *id == conversion.target)
                                .then_some(ty)
                        })
                        .expect("a function signature has a canonical type")
                )
            ));
            dump_expr(module, locals, source, indent + 1, out);
        }
        ExprKind::PtrFromNonZeroULong(operand) => {
            out.push_str(&format!("{pad}PtrFromNonZeroULong : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrToULong(operand) => {
            out.push_str(&format!("{pad}PtrToULong : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrCast(operand) => {
            out.push_str(&format!("{pad}PtrCast : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrLoad { pointer, offset } => {
            out.push_str(&format!("{pad}PtrLoad : {ty}\n"));
            dump_expr(module, locals, pointer, indent + 1, out);
            if let Some(offset) = offset {
                dump_expr(module, locals, offset, indent + 1, out);
            }
        }
        ExprKind::PtrStore {
            pointer,
            offset,
            value,
        } => {
            out.push_str(&format!("{pad}PtrStore : {ty}\n"));
            dump_expr(module, locals, pointer, indent + 1, out);
            if let Some(offset) = offset {
                dump_expr(module, locals, offset, indent + 1, out);
            }
            dump_expr(module, locals, value, indent + 1, out);
        }
        ExprKind::PtrOffset {
            pointer,
            offset,
            subtract,
        } => {
            out.push_str(&format!("{pad}PtrOffset subtract={subtract} : {ty}\n"));
            dump_expr(module, locals, pointer, indent + 1, out);
            dump_expr(module, locals, offset, indent + 1, out);
        }
        ExprKind::AddressOf(Place::Local(local)) => {
            out.push_str(&format!("{pad}AddressOf {} : {ty}\n", locals[*local].name));
        }
        ExprKind::AddressOf(Place::Global(global)) => out.push_str(&format!(
            "{pad}AddressOf global {} : {ty}\n",
            module.globals[*global].name
        )),
        ExprKind::AddressOf(Place::ExternalGlobal { property, .. }) => out.push_str(&format!(
            "{pad}AddressOf external global {property} : {ty}\n"
        )),
        ExprKind::SizeOf(value_ty) => out.push_str(&format!(
            "{pad}SizeOf {} : {ty}\n",
            type_name(module, *value_ty)
        )),
        ExprKind::AlignOf(value_ty) => out.push_str(&format!(
            "{pad}AlignOf {} : {ty}\n",
            type_name(module, *value_ty)
        )),
        ExprKind::FunctionAddress(function) => out.push_str(&format!(
            "{pad}FunctionAddress {} : {ty}\n",
            callable_target_name(module, *function)
        )),
        ExprKind::ForeignCallbackRegister {
            registration,
            closure,
        } => {
            let registration_id = *registration;
            let registration = &module.foreign_callback_registrations[registration_id];
            out.push_str(&format!(
                "{pad}ForeignCallbackRegister registration{} native=function_type{} managed=function_type{} context={} mode=ForeignCallbackMode.{:?} : {ty}\n",
                registration_id.into_raw(),
                registration.native_function_type.into_raw(),
                registration.managed_function_type.into_raw(),
                registration.context_index,
                registration.mode,
            ));
            dump_expr(module, locals, closure, indent + 1, out);
        }
        ExprKind::ForeignCallbackOperation {
            operation,
            callback,
        } => {
            out.push_str(&format!("{pad}ForeignCallback{operation:?} : {ty}\n"));
            dump_expr(module, locals, callback, indent + 1, out);
        }
        ExprKind::FieldAccess { receiver, field } => {
            let field = match field {
                FieldRef::StructField { owner, field } => {
                    format!("field {}", struct_field_index(module, *owner, *field))
                }
                FieldRef::ClassField { owner, field } => {
                    format!("class field {}", class_field_name(module, *owner, *field))
                }
                FieldRef::TupleIndex(index) => format!("_{}", index + 1),
            };
            out.push_str(&format!("{pad}FieldAccess {field} : {ty}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
        }
        ExprKind::InitializingClassFieldAccess { field, .. } => out.push_str(&format!(
            "{pad}InitializingClassFieldAccess {} : {ty}\n",
            initializing_field_name(module, *field)
        )),
        ExprKind::InitializingStructFieldAccess { owner, field } => out.push_str(&format!(
            "{pad}InitializingStructFieldAccess {} : {ty}\n",
            struct_field_index(module, *owner, *field),
        )),
        ExprKind::Call { callee, args, .. } => {
            let callee = callable_target_name(module, *callee);
            out.push_str(&format!("{pad}Call {callee} : {ty}\n"));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::CallableCall {
            callee,
            function_type,
            args,
        } => {
            out.push_str(&format!(
                "{pad}CallableCall function_type{} : {ty}\n",
                function_type.into_raw()
            ));
            dump_expr(module, locals, callee, indent + 1, out);
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::PrimitiveBinary { kind, lhs, rhs } => {
            out.push_str(&format!("{pad}PrimitiveBinary {kind:?} : {ty}\n"));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        ExprKind::PrimitiveUnary { kind, operand } => {
            out.push_str(&format!("{pad}PrimitiveUnary {kind:?} : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::IntegerOperation {
            operation,
            arguments,
        } => {
            match operation {
                IntegerOperation::NoGc { kind, operation } => out.push_str(&format!(
                    "{pad}IntegerOperation {}.{} <no-gc> : {ty}\n",
                    kind.registry_key(),
                    operation.registry_key()
                )),
                IntegerOperation::Managed { kind, operation } => out.push_str(&format!(
                    "{pad}IntegerOperation {}.{} <managed> : {ty}\n",
                    kind.registry_key(),
                    operation.registry_key()
                )),
            }
            match arguments {
                HirIntegerOperationArguments::Unary(operand) => {
                    dump_expr(module, locals, operand, indent + 1, out);
                }
                HirIntegerOperationArguments::Binary { lhs, rhs } => {
                    dump_expr(module, locals, lhs, indent + 1, out);
                    dump_expr(module, locals, rhs, indent + 1, out);
                }
            }
        }
        ExprKind::IntegerConversion {
            conversion,
            operand,
        } => {
            out.push_str(&format!(
                "{pad}IntegerConversion {} -> {} <no-gc> : {ty}\n",
                conversion.source.canonical_name(),
                conversion.target_kind.canonical_name()
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Binary { op, lhs, rhs } => {
            out.push_str(&format!("{pad}Binary {op:?} : {ty}\n"));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        ExprKind::Unary { op, operand } => {
            out.push_str(&format!("{pad}Unary {op:?} : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::MethodCall {
            receiver,
            callee,
            args,
        } => {
            let name = method_callee_name(module, *callee);
            let target = match callee {
                MethodCallee::Callable(_) => name.clone(),
                MethodCallee::Bound(bound) => {
                    let bound = &module.bound_callable_refs[*bound];
                    let via = match bound.source {
                        BoundCallableSource::Class { bound, .. } => {
                            module.class_applications[bound].canonical_type
                        }
                        BoundCallableSource::Interface { bound, .. } => {
                            module.interface_applications[bound].canonical_type
                        }
                    };
                    let receiver = match module.types[bound.receiver_type] {
                        Type::Param(parameter) => format!("T{}", parameter.into_raw()),
                        _ => type_name(module, bound.receiver_type),
                    };
                    format!("bound {receiver} via {} -> {name}", type_name(module, via))
                }
                MethodCallee::DerivedEquality(_) | MethodCallee::ImportedDerivedEquality { .. } => {
                    format!("{name} <derived>")
                }
            };
            out.push_str(&format!("{pad}MethodCall {} : {ty}\n", target));
            dump_expr(module, locals, receiver, indent + 1, out);
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::DirectSuperMethodCall {
            receiver,
            callee,
            args,
        } => {
            let name = method_callee_name(module, *callee);
            out.push_str(&format!("{pad}DirectSuperMethodCall {name} : {ty}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::Box(operand) => {
            out.push_str(&format!("{pad}Box : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Unbox(operand) => {
            out.push_str(&format!("{pad}Unbox : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::ReferenceUpcast(operand) => {
            out.push_str(&format!("{pad}ReferenceUpcast : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::IsInstance { operand, check_ty } => {
            out.push_str(&format!(
                "{pad}IsInstance {} : {ty}\n",
                type_name(module, *check_ty)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Cast {
            operand,
            check_ty,
            optional,
        } => {
            out.push_str(&format!(
                "{pad}Cast {} optional={optional} : {ty}\n",
                type_name(module, *check_ty)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::ArrayLiteral(elements) => {
            out.push_str(&format!("{pad}ArrayLiteral : {ty}\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        ExprKind::ArrayGenerate { count, initializer } => {
            out.push_str(&format!("{pad}ArrayGenerate : {ty}\n"));
            dump_expr(module, locals, count, indent + 1, out);
            dump_expr(module, locals, initializer, indent + 1, out);
        }
        ExprKind::ArrayAssembly(assembly) => {
            out.push_str(&format!("{pad}ArrayAssembly : {ty}\n"));
            for part in &assembly.parts {
                match part {
                    ArrayAssemblyPart::Element(value) => {
                        out.push_str(&format!("{pad}  Element\n"));
                        dump_expr(module, locals, value, indent + 2, out);
                    }
                    ArrayAssemblyPart::CopyArray(value) => {
                        out.push_str(&format!("{pad}  CopyArray\n"));
                        dump_expr(module, locals, value, indent + 2, out);
                    }
                }
            }
        }
        ExprKind::Index {
            access,
            receiver,
            index,
        } => {
            out.push_str(&format!("{pad}Index {access:?} : {ty}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
            dump_expr(module, locals, index, indent + 1, out);
        }
        ExprKind::ArraySet {
            access,
            receiver,
            index,
            value,
        } => {
            out.push_str(&format!("{pad}ArraySet {access:?} : {ty}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
            dump_expr(module, locals, index, indent + 1, out);
            dump_expr(module, locals, value, indent + 1, out);
        }
        ExprKind::ArrayLen(operand) => {
            out.push_str(&format!("{pad}ArrayLen : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::ArrayClone(operand) => {
            out.push_str(&format!("{pad}ArrayClone : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }

        ExprKind::ConstructorReceiver => {
            out.push_str(&format!("{pad}ConstructorReceiver : {ty}\n"))
        }
        ExprKind::SomeWrap(operand) => {
            out.push_str(&format!("{pad}SomeWrap : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::NoneLiteral => out.push_str(&format!("{pad}NoneLiteral : {ty}\n")),
        ExprKind::IsSome(operand) => {
            out.push_str(&format!("{pad}IsSome : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Unwrap {
            operand,
            trap_on_none,
        } => {
            out.push_str(&format!("{pad}Unwrap trap={trap_on_none} : {ty}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
    }
}
