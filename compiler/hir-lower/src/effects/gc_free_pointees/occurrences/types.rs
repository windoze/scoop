use scoop_hir as hir;

use crate::Lowerer;

mod callables;
use callables::collect_callable_reference_types;
pub(in super::super) use callables::{
    collect_callable_target_types, collect_callable_types, collect_imported_method_callee_types,
    collect_imported_reference_target_types, collect_method_callee_types,
};

pub(in super::super) fn collect_body_types(
    lowerer: &Lowerer,
    body: &hir::Body,
    out: &mut Vec<hir::TypeId>,
) {
    out.extend(body.locals.iter().map(|(_, local)| local.ty));
    collect_statement_types(lowerer, &body.statements, out);
}

pub(in super::super) fn collect_statement_types(
    lowerer: &Lowerer,
    statements: &[hir::Statement],
    out: &mut Vec<hir::TypeId>,
) {
    for statement in statements {
        match &statement.kind {
            hir::StatementKind::GenericDelegateEnsure(reference) => {
                out.extend(reference.arguments.iter().copied());
            }
            hir::StatementKind::Expr(expression) | hir::StatementKind::Throw(expression) => {
                collect_expr_types(lowerer, expression, out)
            }
            hir::StatementKind::Return { value } => {
                if let Some(value) = value {
                    collect_expr_types(lowerer, value, out);
                }
            }
            hir::StatementKind::ValDecl { pattern, init } => {
                collect_pattern_types(lowerer, pattern, out);
                collect_expr_types(lowerer, init, out);
            }
            hir::StatementKind::Assign { target, value } => {
                match target {
                    hir::AssignTarget::GenericDelegateStorage(reference) => {
                        out.extend(reference.arguments.iter().copied());
                    }
                    hir::AssignTarget::Index { array, index } => {
                        collect_expr_types(lowerer, array, out);
                        collect_expr_types(lowerer, index, out);
                    }
                    hir::AssignTarget::Field { receiver, field } => {
                        collect_expr_types(lowerer, receiver, out);
                        collect_field_ref_types(*field, out);
                    }
                    hir::AssignTarget::Local(_)
                    | hir::AssignTarget::Global(_)
                    | hir::AssignTarget::SingletonPublishedRoot(_) => {}
                    hir::AssignTarget::InitializingClassField { field, .. } => {
                        out.push(field.owner);
                    }
                }
                collect_expr_types(lowerer, value, out);
            }
            hir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                collect_expr_types(lowerer, cond, out);
                collect_statement_types(lowerer, then_body, out);
                if let Some(else_body) = else_body {
                    collect_statement_types(lowerer, else_body, out);
                }
            }
            hir::StatementKind::While {
                target: _,
                condition_setup,
                cond,
                body,
            } => {
                collect_statement_types(lowerer, condition_setup, out);
                collect_expr_types(lowerer, cond, out);
                collect_statement_types(lowerer, body, out);
            }
            hir::StatementKind::When(when) => {
                collect_expr_types(lowerer, &when.subject, out);
                for arm in &when.arms {
                    collect_pattern_types(lowerer, &arm.pattern, out);
                    if let Some(guard) = &arm.guard {
                        collect_statement_types(lowerer, &guard.setup, out);
                        collect_expr_types(lowerer, &guard.condition, out);
                    }
                    collect_statement_types(lowerer, &arm.body, out);
                }
                if let hir::WhenFallback::Else(body) = &when.fallback {
                    collect_statement_types(lowerer, body, out);
                }
            }
            hir::StatementKind::Try(try_) => {
                collect_statement_types(lowerer, &try_.body, out);
                for catch in &try_.catches {
                    out.push(catch.ty);
                    collect_statement_types(lowerer, &catch.body, out);
                }
                if let Some(body) = &try_.finally_body {
                    collect_statement_types(lowerer, body, out);
                }
            }
            hir::StatementKind::InitializationEnsure(_)
            | hir::StatementKind::LocalFunction(_)
            | hir::StatementKind::Break { .. }
            | hir::StatementKind::Continue { .. } => {}
        }
    }
}

fn collect_pattern_types(lowerer: &Lowerer, pattern: &hir::Pattern, out: &mut Vec<hir::TypeId>) {
    match pattern {
        hir::Pattern::Literal {
            value,
            equality,
            subject_ty,
        } => {
            out.push(*subject_ty);
            if let hir::LiteralPatternEquality::Ordinary { equals } = equality {
                collect_callable_target_types(lowerer, *equals, out);
            }
            collect_expr_types(lowerer, value, out);
        }
        hir::Pattern::Variant {
            application,
            fields,
            ..
        } => {
            out.push(application.owner);
            for (_, field) in fields {
                collect_pattern_types(lowerer, field, out);
            }
        }
        hir::Pattern::Struct { owner, fields } => {
            out.push(*owner);
            for (_, field) in fields {
                collect_pattern_types(lowerer, field, out);
            }
        }
        hir::Pattern::Tuple(elements) => {
            for element in elements {
                collect_pattern_types(lowerer, element, out);
            }
        }
        hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => {}
    }
}

pub(in super::super) fn collect_expr_types(
    lowerer: &Lowerer,
    expression: &hir::Expr,
    out: &mut Vec<hir::TypeId>,
) {
    out.push(expression.ty);
    use hir::ExprKind;
    match &expression.kind {
        ExprKind::GenericDelegateStorageRead(reference) => {
            out.extend(reference.arguments.iter().copied());
        }
        ExprKind::TupleLiteral(values) | ExprKind::ArrayLiteral(values) => {
            for value in values {
                collect_expr_types(lowerer, value, out);
            }
        }
        ExprKind::StructInit { constructor, args } => {
            let application = lowerer.struct_constructor_applications[*constructor].owner;
            out.push(lowerer.struct_applications[application].canonical_type);
            for argument in args {
                collect_expr_types(lowerer, argument, out);
            }
        }
        ExprKind::StructConstruct {
            application,
            fields,
        } => {
            out.push(lowerer.struct_applications[*application].canonical_type);
            for field in fields {
                collect_expr_types(lowerer, field, out);
            }
        }
        ExprKind::ClassInit { constructor, args } => {
            let application = lowerer.class_constructor_applications[*constructor].owner;
            out.push(lowerer.class_applications[application].canonical_type);
            for argument in args {
                collect_expr_types(lowerer, argument, out);
            }
        }
        ExprKind::VariantConstruct { variant, args } => {
            out.push(variant.owner);
            for argument in args {
                collect_expr_types(lowerer, argument, out);
            }
        }
        ExprKind::VariantTest { operand, variant } => {
            out.push(variant.owner);
            collect_expr_types(lowerer, operand, out);
        }
        ExprKind::VariantPayloadProject { operand, field } => {
            out.push(field.variant.owner);
            collect_expr_types(lowerer, operand, out);
        }
        ExprKind::ArrayAssembly(assembly) => {
            out.push(assembly.element_type);
            out.push(assembly.result_type);
            for part in &assembly.parts {
                match part {
                    hir::ArrayAssemblyPart::Element(value)
                    | hir::ArrayAssemblyPart::CopyArray(value) => {
                        collect_expr_types(lowerer, value, out)
                    }
                }
            }
        }
        ExprKind::FunctionCoercion {
            source,
            coercion,
            target_type,
        } => {
            let coercion = &lowerer.function_coercions[*coercion];
            collect_function_type_types(lowerer, coercion.source, out);
            collect_function_type_types(lowerer, coercion.target, out);
            collect_function_type_types(lowerer, *target_type, out);
            collect_expr_types(lowerer, source, out);
        }
        ExprKind::PtrFromNonZeroULong(source)
        | ExprKind::PtrToULong(source)
        | ExprKind::PtrCast(source)
        | ExprKind::Box(source)
        | ExprKind::Unbox(source)
        | ExprKind::ReferenceUpcast(source)
        | ExprKind::ArrayLen(source)
        | ExprKind::ArrayClone(source)
        | ExprKind::PrimitiveUnary {
            operand: source, ..
        }
        | ExprKind::IntegerConversion {
            operand: source, ..
        }
        | ExprKind::Unary {
            operand: source, ..
        }
        | ExprKind::SomeWrap(source)
        | ExprKind::IsSome(source)
        | ExprKind::Unwrap {
            operand: source, ..
        } => collect_expr_types(lowerer, source, out),
        ExprKind::PtrLoad { pointer, offset } => {
            collect_expr_types(lowerer, pointer, out);
            if let Some(offset) = offset {
                collect_expr_types(lowerer, offset, out);
            }
        }
        ExprKind::PtrStore {
            pointer,
            offset,
            value,
        } => {
            collect_expr_types(lowerer, pointer, out);
            if let Some(offset) = offset {
                collect_expr_types(lowerer, offset, out);
            }
            collect_expr_types(lowerer, value, out);
        }
        ExprKind::PtrOffset {
            pointer, offset, ..
        }
        | ExprKind::Index {
            receiver: pointer,
            index: offset,
            ..
        } => {
            collect_expr_types(lowerer, pointer, out);
            collect_expr_types(lowerer, offset, out);
        }
        ExprKind::ArraySet {
            receiver,
            index,
            value,
            ..
        } => {
            collect_expr_types(lowerer, receiver, out);
            collect_expr_types(lowerer, index, out);
            collect_expr_types(lowerer, value, out);
        }
        ExprKind::ForeignCallbackRegister {
            registration,
            closure,
        } => {
            let registration = &lowerer.foreign_callback_registrations[*registration];
            collect_function_type_types(lowerer, registration.native_function_type, out);
            collect_function_type_types(lowerer, registration.managed_function_type, out);
            collect_expr_types(lowerer, closure, out);
        }
        ExprKind::ForeignCallbackOperation { callback, .. } => {
            collect_expr_types(lowerer, callback, out)
        }
        ExprKind::FieldAccess { receiver, field } => {
            collect_field_ref_types(*field, out);
            collect_expr_types(lowerer, receiver, out);
        }
        ExprKind::MethodCall {
            receiver,
            callee,
            args,
        }
        | ExprKind::DirectSuperMethodCall {
            receiver,
            callee,
            args,
        } => {
            collect_method_callee_types(lowerer, *callee, out);
            collect_expr_types(lowerer, receiver, out);
            for argument in args {
                collect_expr_types(lowerer, argument, out);
            }
        }
        ExprKind::IsInstance { operand, check_ty }
        | ExprKind::Cast {
            operand, check_ty, ..
        } => {
            out.push(*check_ty);
            collect_expr_types(lowerer, operand, out);
        }
        ExprKind::Call { callee, args, .. } => {
            collect_callable_types(lowerer, *callee, out);
            for argument in args {
                collect_expr_types(lowerer, argument, out);
            }
        }
        ExprKind::ImportedConstructorInit { application, args } => {
            out.push(lowerer.imported_constructor_applications[*application].owner);
            for argument in args {
                collect_expr_types(lowerer, argument, out);
            }
        }
        ExprKind::ImportedGenericCall {
            application,
            args,
            receiver,
            ..
        } => {
            out.extend(
                lowerer.imported_generic_applications[*application]
                    .arguments
                    .substitution(
                        &lowerer.types,
                        &lowerer.enum_applications,
                        &lowerer.struct_applications,
                    ),
            );
            if let hir::SourceCallReceiver::Receiver { static_type } = receiver {
                out.push(*static_type);
            }
            for argument in args {
                collect_expr_types(lowerer, argument, out);
            }
        }
        ExprKind::ImportedDependencyCall { args, .. } => {
            for argument in args {
                collect_expr_types(lowerer, argument, out);
            }
        }
        ExprKind::LocalFunctionCall {
            callee,
            captures,
            args,
            ..
        } => {
            collect_callable_types(lowerer, *callee, out);
            for capture in captures {
                collect_expr_types(lowerer, capture, out);
            }
            for argument in args {
                collect_expr_types(lowerer, argument, out);
            }
        }
        ExprKind::CallableCall {
            callee,
            function_type,
            args,
        } => {
            collect_function_type_types(lowerer, *function_type, out);
            collect_expr_types(lowerer, callee, out);
            for argument in args {
                collect_expr_types(lowerer, argument, out);
            }
        }
        ExprKind::PrimitiveBinary { lhs, rhs, .. } | ExprKind::Binary { lhs, rhs, .. } => {
            collect_expr_types(lowerer, lhs, out);
            collect_expr_types(lowerer, rhs, out);
        }
        ExprKind::IntegerOperation { arguments, .. } => match arguments {
            hir::HirIntegerOperationArguments::Unary(operand) => {
                collect_expr_types(lowerer, operand, out)
            }
            hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                collect_expr_types(lowerer, lhs, out);
                collect_expr_types(lowerer, rhs, out);
            }
        },
        ExprKind::SizeOf(ty) | ExprKind::AlignOf(ty) => out.push(*ty),
        ExprKind::Lambda(lambda) => {
            let lambda = &lowerer.lambdas[*lambda];
            collect_function_type_types(lowerer, lambda.function_type, out);
            if let hir::CallableBodyTypeArguments::Explicit(arguments) = &lambda.body_type_arguments
            {
                out.extend(arguments.iter().copied());
            }
        }
        ExprKind::ImportedClosure(closure) => {
            collect_function_type_types(lowerer, closure.function_type, out);
            out.extend(
                lowerer.imported_generic_applications[closure.application]
                    .arguments
                    .substitution(
                        &lowerer.types,
                        &lowerer.enum_applications,
                        &lowerer.struct_applications,
                    ),
            );
            for capture in &closure.captures {
                out.push(capture.ty);
                collect_expr_types(lowerer, &capture.source, out);
            }
        }
        ExprKind::ImportedMethodCall {
            receiver,
            callee,
            args,
        } => {
            collect_imported_method_callee_types(lowerer, callee, out);
            collect_expr_types(lowerer, receiver, out);
            for arg in args {
                collect_expr_types(lowerer, arg, out);
            }
        }
        ExprKind::ImportedCallableReference(reference) => {
            collect_function_type_types(lowerer, reference.function_type, out);
            out.extend(reference.owner_type_arguments.iter().copied());
            collect_imported_reference_target_types(lowerer, &reference.target, out);
            if let Some(receiver) = reference.target.receiver() {
                collect_expr_types(lowerer, receiver, out);
            }
            for capture in &reference.captures {
                out.push(capture.ty);
                collect_expr_types(lowerer, &capture.source, out);
            }
        }
        ExprKind::AnonymousFunction(function) => {
            let function = &lowerer.anonymous_functions[*function];
            collect_function_type_types(lowerer, function.function_type, out);
            if let hir::CallableBodyTypeArguments::Explicit(arguments) =
                &function.body_type_arguments
            {
                out.extend(arguments.iter().copied());
            }
        }
        ExprKind::CallableReference(reference) => {
            collect_callable_reference_types(lowerer, *reference, out);
        }
        ExprKind::InitializingClassFieldAccess { field, .. } => {
            out.push(field.owner);
        }
        ExprKind::InitializingStructFieldAccess { owner, .. } => {
            out.push(*owner);
        }
        ExprKind::StringLiteral { .. }
        | ExprKind::IntegerLiteral(_)
        | ExprKind::BoolLiteral(_)
        | ExprKind::UnitLiteral
        | ExprKind::ConstructorReceiver
        | ExprKind::ConstructorParam(_)
        | ExprKind::Local(_)
        | ExprKind::GlobalRead(_)
        | ExprKind::SingletonValue(_)
        | ExprKind::ImportedSingletonValue(_)
        | ExprKind::Capture(_)
        | ExprKind::AddressOf(_)
        | ExprKind::FunctionAddress(_)
        | ExprKind::NoneLiteral => {}
    }
}

pub(in super::super) fn collect_function_type_types(
    lowerer: &Lowerer,
    function: hir::FunctionTypeId,
    out: &mut Vec<hir::TypeId>,
) {
    let function = &lowerer.function_types[function];
    out.extend(function.parameter_types.iter().copied());
    out.push(function.return_type);
}

pub(in super::super) fn collect_field_ref_types(field: hir::FieldRef, out: &mut Vec<hir::TypeId>) {
    match field {
        hir::FieldRef::StructField { owner, .. } | hir::FieldRef::ClassField { owner, .. } => {
            out.push(owner)
        }
        hir::FieldRef::TupleIndex(_) => {}
    }
}
