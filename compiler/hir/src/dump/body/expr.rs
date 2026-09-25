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
        ExprKind::StringLiteral { value, .. } => {
            out.push_str(&format!("{pad}StringLiteral {value:?} : {ty}\n"));
        }
        ExprKind::IntegerLiteral(value) => {
            out.push_str(&format!("{pad}IntegerLiteral {value} : {ty}\n"));
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
            let name = &module.classes[application.template].name;
            let arguments = application
                .arguments
                .iter()
                .map(|ty| type_name(module, *ty))
                .collect::<Vec<_>>();
            let constructed = if arguments.is_empty() {
                name.clone()
            } else {
                format!("{name}<{}>", arguments.join(", "))
            };
            out.push_str(&format!("{pad}ClassInit {} : {ty}\n", constructed));
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
            let application = &module.struct_applications[constructor.owner];
            out.push_str(&format!(
                "{pad}StructInit {} : {ty}\n",
                module.structs[application.template].name
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
            let application = &module.enum_applications[variant.application()];
            let decl = &module.enums[application.template];
            let type_args = if application.arguments.is_empty() {
                String::new()
            } else {
                let args: Vec<String> = application
                    .arguments
                    .iter()
                    .map(|t| type_name(module, *t))
                    .collect();
                format!("<{}>", args.join(", "))
            };
            out.push_str(&format!(
                "{pad}VariantConstruct {}.{}{type_args} : {ty}\n",
                decl.name,
                decl.variants[variant.local_index() as usize].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::VariantTest { operand, variant } => {
            let application = &module.enum_applications[variant.application()];
            let declaration = &module.enums[application.template];
            out.push_str(&format!(
                "{pad}VariantTest {}.{} : {ty}\n",
                type_name(module, application.canonical_type),
                declaration.variants[variant.local_index() as usize].name
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::VariantPayloadProject { operand, field } => {
            let variant = field.variant();
            let application = &module.enum_applications[variant.application()];
            let declaration = &module.enums[application.template];
            let payload = &declaration.variants[variant.local_index() as usize];
            out.push_str(&format!(
                "{pad}VariantPayloadProject {}.{}.{} : {ty}\n",
                type_name(module, application.canonical_type),
                payload.name,
                payload.fields[field.local_index() as usize].name
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
            let declaration = module.singleton_values[*value].declaration;
            out.push_str(&format!(
                "{pad}SingletonValue {} : {ty}\n",
                module.objects[declaration].name
            ));
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
                module.functions[lambda.function].name,
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
                module.functions[anonymous.function].name,
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
            let (function, receiver) = match &reference.target {
                CallableReferenceTarget::Named(callable) => {
                    (callable_function(module, *callable), None)
                }
                CallableReferenceTarget::Local { callee, .. } => {
                    (callable_function(module, *callee), None)
                }
                CallableReferenceTarget::BoundMember { receiver, callee } => (
                    method_callee_function(module, *callee),
                    Some(receiver.as_ref()),
                ),
                CallableReferenceTarget::BoundExtension { receiver, callee } => {
                    (callable_function(module, *callee), Some(receiver.as_ref()))
                }
            };
            out.push_str(&format!(
                "{pad}CallableReference reference{} target={} captures={} : {ty}\n",
                id.into_raw(),
                module.functions[function].name,
                reference.captures.len()
            ));
            if let Some(receiver) = receiver {
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
            module.functions[*function].name
        )),
        ExprKind::ForeignCallbackRegister {
            registration,
            closure,
        } => {
            let registration_id = *registration;
            let registration = &module.foreign_callback_registrations[registration_id];
            let mode_application = &module.enum_applications[registration.mode.application()];
            let mode_declaration = &module.enums[mode_application.template];
            let mode = &mode_declaration.variants[registration.mode.local_index() as usize].name;
            out.push_str(&format!(
                "{pad}ForeignCallbackRegister registration{} native=function_type{} managed=function_type{} context={} mode={}.{} : {ty}\n",
                registration_id.into_raw(),
                registration.native_function_type.into_raw(),
                registration.managed_function_type.into_raw(),
                registration.context_index,
                type_name(module, mode_application.canonical_type),
                mode,
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
                FieldRef::StructField(field) => format!("field {}", field.local_index()),
                FieldRef::TupleIndex(index) => format!("_{}", index + 1),
                FieldRef::ClassField { field, .. } => {
                    format!(
                        "class field {}",
                        module.properties[module.class_fields[*field].property].name
                    )
                }
            };
            out.push_str(&format!("{pad}FieldAccess {field} : {ty}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
        }
        ExprKind::InitializingClassFieldAccess { field, .. } => out.push_str(&format!(
            "{pad}InitializingClassFieldAccess {} : {ty}\n",
            module.properties[module.class_fields[*field].property].name
        )),
        ExprKind::InitializingStructFieldAccess { index, .. } => out.push_str(&format!(
            "{pad}InitializingStructFieldAccess {index} : {ty}\n"
        )),
        ExprKind::Call { callee, args, .. } => {
            let (function, type_args) = callable_dump_parts(module, *callee);
            let callee = &module.functions[function];
            let type_args = if type_args.is_empty() {
                String::new()
            } else {
                let args: Vec<String> = type_args.iter().map(|t| type_name(module, *t)).collect();
                format!("<{}>", args.join(", "))
            };
            out.push_str(&format!("{pad}Call {}{type_args} : {ty}\n", callee.name));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::LocalFunctionCall {
            local_function,
            callee,
            captures,
            args,
        } => {
            let (function, type_args) = callable_dump_parts(module, *callee);
            let type_args = if type_args.is_empty() {
                String::new()
            } else {
                let args: Vec<String> = type_args.iter().map(|t| type_name(module, *t)).collect();
                format!("<{}>", args.join(", "))
            };
            out.push_str(&format!(
                "{pad}LocalFunctionCall local{} {}{type_args} captures={} : {ty}\n",
                local_function.into_raw(),
                module.functions[function].name,
                captures.len()
            ));
            for capture in captures {
                dump_expr(module, locals, capture, indent + 1, out);
            }
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
            let function = method_callee_function(module, *callee);
            let target = match callee {
                MethodCallee::Callable(_) => module.functions[function].name.clone(),
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
                    format!(
                        "bound T{} via {} -> {}",
                        bound.receiver_parameter.into_raw(),
                        type_name(module, via),
                        module.functions[function].name
                    )
                }
                MethodCallee::DerivedEquality(_) => {
                    format!("{} <derived>", module.functions[function].name)
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
            let function = method_callee_function(module, *callee);
            out.push_str(&format!(
                "{pad}DirectSuperMethodCall {} : {ty}\n",
                module.functions[function].name
            ));
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

        ExprKind::ImportedDependencyCall { callee, args, .. } => {
            out.push_str(&format!(
                "{pad}ImportedDependencyCall #{} : {ty}\n",
                callee.into_raw().into_u32()
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
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
