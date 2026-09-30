use scoop_hir as hir;

use crate::Lowerer;

use super::TypeOccurrence;
use super::types::{
    collect_callable_target_types, collect_callable_types, collect_field_ref_types,
    collect_method_callee_types, collect_reference_target_types,
};

/// Collect source-backed type occurrences for diagnostics. Unlike the
/// inference walkers, this deliberately omits local-arena types and records
/// every expression at its concrete evaluation origin. Types copied into
/// several constructor bodies therefore keep one source location, while two
/// actual applications of the same type remain distinct occurrences.
pub(in super::super) fn collect_body_type_occurrences(
    lowerer: &Lowerer,
    body: &hir::Body,
    file: usize,
    out: &mut Vec<TypeOccurrence>,
) {
    collect_statement_type_occurrences(lowerer, &body.statements, file, out);
}

pub(in super::super) fn collect_default_type_occurrences(
    lowerer: &Lowerer,
    body: &hir::ExportDefaultExpr,
    out: &mut Vec<TypeOccurrence>,
) {
    let file = body.origin.file as usize;
    collect_statement_type_occurrences(lowerer, &body.statements, file, out);
    collect_expr_type_occurrences(lowerer, &body.value, out);
}

pub(in super::super) fn collect_statement_type_occurrences(
    lowerer: &Lowerer,
    statements: &[hir::Statement],
    file: usize,
    out: &mut Vec<TypeOccurrence>,
) {
    for statement in statements {
        match &statement.kind {
            hir::StatementKind::GenericDelegateEnsure(reference) => push_types_at(
                file,
                statement.span,
                |types| types.extend(reference.arguments.iter().copied()),
                out,
            ),
            hir::StatementKind::InitializationEnsure(_)
            | hir::StatementKind::LocalFunction(_)
            | hir::StatementKind::Break { .. }
            | hir::StatementKind::Continue { .. } => {}
            hir::StatementKind::Expr(expression) | hir::StatementKind::Throw(expression) => {
                collect_expr_type_occurrences(lowerer, expression, out);
            }
            hir::StatementKind::Return { value } => {
                if let Some(value) = value {
                    collect_expr_type_occurrences(lowerer, value, out);
                }
            }
            hir::StatementKind::ValDecl { pattern, init } => {
                collect_pattern_type_occurrences(lowerer, pattern, file, statement.span, out);
                collect_expr_type_occurrences(lowerer, init, out);
            }
            hir::StatementKind::Assign { target, value } => {
                match target {
                    hir::AssignTarget::GenericDelegateStorage(reference) => push_types_at(
                        file,
                        statement.span,
                        |types| types.extend(reference.arguments.iter().copied()),
                        out,
                    ),
                    hir::AssignTarget::Local(_)
                    | hir::AssignTarget::Global(_)
                    | hir::AssignTarget::SingletonPublishedRoot(_) => {}
                    hir::AssignTarget::Index { array, index } => {
                        collect_expr_type_occurrences(lowerer, array, out);
                        collect_expr_type_occurrences(lowerer, index, out);
                    }
                    hir::AssignTarget::Field { receiver, field } => {
                        push_types_at(
                            file,
                            statement.span,
                            |types| collect_field_ref_types(*field, types),
                            out,
                        );
                        collect_expr_type_occurrences(lowerer, receiver, out);
                    }
                    hir::AssignTarget::InitializingClassField { field, origin, .. } => {
                        let evaluation = origin.concrete().evaluation;
                        out.push(TypeOccurrence {
                            ty: field.owner,
                            file: evaluation.file as usize,
                            span: evaluation.span,
                        });
                    }
                }
                collect_expr_type_occurrences(lowerer, value, out);
            }
            hir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                collect_expr_type_occurrences(lowerer, cond, out);
                collect_statement_type_occurrences(lowerer, then_body, file, out);
                if let Some(else_body) = else_body {
                    collect_statement_type_occurrences(lowerer, else_body, file, out);
                }
            }
            hir::StatementKind::While {
                target: _,
                condition_setup,
                cond,
                body,
            } => {
                collect_statement_type_occurrences(lowerer, condition_setup, file, out);
                collect_expr_type_occurrences(lowerer, cond, out);
                collect_statement_type_occurrences(lowerer, body, file, out);
            }
            hir::StatementKind::When(when) => {
                collect_expr_type_occurrences(lowerer, &when.subject, out);
                for arm in &when.arms {
                    collect_pattern_type_occurrences(lowerer, &arm.pattern, file, arm.span, out);
                    if let Some(guard) = &arm.guard {
                        collect_statement_type_occurrences(lowerer, &guard.setup, file, out);
                        collect_expr_type_occurrences(lowerer, &guard.condition, out);
                    }
                    collect_statement_type_occurrences(lowerer, &arm.body, file, out);
                }
                if let hir::WhenFallback::Else(body) = &when.fallback {
                    collect_statement_type_occurrences(lowerer, body, file, out);
                }
            }
            hir::StatementKind::Try(try_) => {
                collect_statement_type_occurrences(lowerer, &try_.body, file, out);
                for catch in &try_.catches {
                    out.push(TypeOccurrence {
                        ty: catch.ty,
                        file,
                        span: catch.span,
                    });
                    collect_statement_type_occurrences(lowerer, &catch.body, file, out);
                }
                if let Some(finally_body) = &try_.finally_body {
                    collect_statement_type_occurrences(lowerer, finally_body, file, out);
                }
            }
        }
    }
}

fn collect_pattern_type_occurrences(
    lowerer: &Lowerer,
    pattern: &hir::Pattern,
    file: usize,
    span: scoop_ast::Span,
    out: &mut Vec<TypeOccurrence>,
) {
    match pattern {
        hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => {}
        hir::Pattern::Literal {
            value,
            equality,
            subject_ty,
        } => {
            push_type_at_expression(*subject_ty, value, out);
            if let hir::LiteralPatternEquality::Ordinary { equals } = equality {
                push_types_at_expression(
                    value,
                    |types| collect_callable_target_types(lowerer, *equals, types),
                    out,
                );
            }
            collect_expr_type_occurrences(lowerer, value, out);
        }
        hir::Pattern::Variant {
            application,
            fields,
            ..
        } => {
            out.push(TypeOccurrence {
                ty: application.owner,
                file,
                span,
            });
            for (_, field) in fields {
                collect_pattern_type_occurrences(lowerer, field, file, span, out);
            }
        }
        hir::Pattern::Tuple(elements) => {
            for element in elements {
                collect_pattern_type_occurrences(lowerer, element, file, span, out);
            }
        }
        hir::Pattern::Struct { owner, fields } => {
            out.push(TypeOccurrence {
                ty: *owner,
                file,
                span,
            });
            for (_, field) in fields {
                collect_pattern_type_occurrences(lowerer, field, file, span, out);
            }
        }
    }
}

pub(in super::super) fn collect_expr_type_occurrences(
    lowerer: &Lowerer,
    expression: &hir::Expr,
    out: &mut Vec<TypeOccurrence>,
) {
    push_type_at_expression(expression.ty, expression, out);

    use hir::ExprKind;
    match &expression.kind {
        ExprKind::GenericDelegateStorageRead(reference) => {
            for argument in reference.arguments.iter() {
                push_type_at_expression(*argument, expression, out);
            }
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
        | ExprKind::Capture(_)
        | ExprKind::AddressOf(_)
        | ExprKind::FunctionAddress(_)
        | ExprKind::NoneLiteral => {}
        ExprKind::TupleLiteral(values) | ExprKind::ArrayLiteral(values) => {
            for value in values {
                collect_expr_type_occurrences(lowerer, value, out);
            }
        }
        ExprKind::StructInit { constructor, args } => {
            let application = lowerer.struct_constructor_applications[*constructor].owner;
            push_type_at_expression(
                lowerer.struct_applications[application].canonical_type,
                expression,
                out,
            );
            for argument in args {
                collect_expr_type_occurrences(lowerer, argument, out);
            }
        }
        ExprKind::StructConstruct {
            application,
            fields,
        } => {
            push_type_at_expression(
                lowerer.struct_applications[*application].canonical_type,
                expression,
                out,
            );
            for field in fields {
                collect_expr_type_occurrences(lowerer, field, out);
            }
        }
        ExprKind::ClassInit { constructor, args } => {
            let application = lowerer.class_constructor_applications[*constructor].owner;
            push_type_at_expression(
                lowerer.class_applications[application].canonical_type,
                expression,
                out,
            );
            for argument in args {
                collect_expr_type_occurrences(lowerer, argument, out);
            }
        }
        ExprKind::VariantConstruct { variant, args } => {
            push_type_at_expression(variant.owner, expression, out);
            for argument in args {
                collect_expr_type_occurrences(lowerer, argument, out);
            }
        }
        ExprKind::VariantTest { operand, variant } => {
            push_type_at_expression(variant.owner, expression, out);
            collect_expr_type_occurrences(lowerer, operand, out);
        }
        ExprKind::VariantPayloadProject { operand, field } => {
            push_type_at_expression(field.variant.owner, expression, out);
            collect_expr_type_occurrences(lowerer, operand, out);
        }
        ExprKind::Lambda(id) => {
            let closure = &lowerer.lambdas[*id];
            push_types_at_expression(
                expression,
                |types| {
                    if let hir::CallableBodyTypeArguments::Explicit(arguments) =
                        &closure.body_type_arguments
                    {
                        types.extend(arguments.iter().copied());
                    }
                    types.extend(closure.captures.iter().map(|capture| capture.ty));
                },
                out,
            );
            for capture in &closure.captures {
                collect_expr_type_occurrences(lowerer, &capture.source, out);
            }
        }
        ExprKind::AnonymousFunction(id) => {
            let closure = &lowerer.anonymous_functions[*id];
            push_types_at_expression(
                expression,
                |types| {
                    if let hir::CallableBodyTypeArguments::Explicit(arguments) =
                        &closure.body_type_arguments
                    {
                        types.extend(arguments.iter().copied());
                    }
                    types.extend(closure.captures.iter().map(|capture| capture.ty));
                },
                out,
            );
            for capture in &closure.captures {
                collect_expr_type_occurrences(lowerer, &capture.source, out);
            }
        }
        ExprKind::CallableReference(reference) => {
            let reference = &lowerer.callable_references[*reference];
            for capture in &reference.captures {
                collect_expr_type_occurrences(lowerer, &capture.source, out);
            }
            push_types_at_expression(
                expression,
                |types| {
                    collect_reference_target_types(lowerer, &reference.target, types);
                    types.extend(reference.owner_type_arguments.iter().copied());
                    types.extend(reference.captures.iter().map(|capture| capture.ty));
                },
                out,
            );
            if let Some(receiver) = reference.target.receiver() {
                collect_expr_type_occurrences(lowerer, receiver, out);
            }
        }
        ExprKind::FunctionCoercion { source, .. }
        | ExprKind::PtrFromNonZeroULong(source)
        | ExprKind::PtrToULong(source)
        | ExprKind::PtrCast(source)
        | ExprKind::Box(source)
        | ExprKind::Unbox(source)
        | ExprKind::ReferenceUpcast(source)
        | ExprKind::IsInstance {
            operand: source, ..
        }
        | ExprKind::Cast {
            operand: source, ..
        }
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
        } => collect_expr_type_occurrences(lowerer, source, out),
        ExprKind::PtrLoad { pointer, offset } => {
            collect_expr_type_occurrences(lowerer, pointer, out);
            if let Some(offset) = offset {
                collect_expr_type_occurrences(lowerer, offset, out);
            }
        }
        ExprKind::PtrStore {
            pointer,
            offset,
            value,
        } => {
            collect_expr_type_occurrences(lowerer, pointer, out);
            if let Some(offset) = offset {
                collect_expr_type_occurrences(lowerer, offset, out);
            }
            collect_expr_type_occurrences(lowerer, value, out);
        }
        ExprKind::PtrOffset {
            pointer, offset, ..
        }
        | ExprKind::Index {
            receiver: pointer,
            index: offset,
            ..
        }
        | ExprKind::PrimitiveBinary {
            lhs: pointer,
            rhs: offset,
            ..
        }
        | ExprKind::Binary {
            lhs: pointer,
            rhs: offset,
            ..
        } => {
            collect_expr_type_occurrences(lowerer, pointer, out);
            collect_expr_type_occurrences(lowerer, offset, out);
        }
        ExprKind::SizeOf(ty) | ExprKind::AlignOf(ty) => {
            push_type_at_expression(*ty, expression, out);
        }
        ExprKind::ForeignCallbackRegister {
            registration,
            closure,
        } => {
            let registration = &lowerer.foreign_callback_registrations[*registration];
            push_type_at_expression(
                lowerer.function_types[registration.native_function_type].canonical_type,
                expression,
                out,
            );
            push_type_at_expression(
                lowerer.function_types[registration.managed_function_type].canonical_type,
                expression,
                out,
            );
            collect_expr_type_occurrences(lowerer, closure, out);
        }
        ExprKind::ForeignCallbackOperation { callback, .. } => {
            collect_expr_type_occurrences(lowerer, callback, out);
        }
        ExprKind::FieldAccess { receiver, field } => {
            push_types_at_expression(
                expression,
                |types| collect_field_ref_types(*field, types),
                out,
            );
            collect_expr_type_occurrences(lowerer, receiver, out);
        }
        ExprKind::InitializingClassFieldAccess { field, .. } => {
            push_type_at_expression(field.owner, expression, out);
        }
        ExprKind::InitializingStructFieldAccess { owner, .. } => {
            push_type_at_expression(*owner, expression, out);
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
            push_types_at_expression(
                expression,
                |types| collect_method_callee_types(lowerer, *callee, types),
                out,
            );
            collect_expr_type_occurrences(lowerer, receiver, out);
            for argument in args {
                collect_expr_type_occurrences(lowerer, argument, out);
            }
        }
        ExprKind::ArrayAssembly(assembly) => {
            push_type_at_expression(assembly.element_type, expression, out);
            push_type_at_expression(assembly.result_type, expression, out);
            for part in &assembly.parts {
                match part {
                    hir::ArrayAssemblyPart::Element(value)
                    | hir::ArrayAssemblyPart::CopyArray(value) => {
                        collect_expr_type_occurrences(lowerer, value, out);
                    }
                }
            }
        }
        ExprKind::ArraySet {
            receiver,
            index,
            value,
            ..
        } => {
            collect_expr_type_occurrences(lowerer, receiver, out);
            collect_expr_type_occurrences(lowerer, index, out);
            collect_expr_type_occurrences(lowerer, value, out);
        }
        ExprKind::Call {
            callee,
            args,
            receiver,
            ..
        } => {
            push_types_at_expression(
                expression,
                |types| {
                    collect_callable_target_types(lowerer, *callee, types);
                    if let hir::SourceCallReceiver::Receiver { static_type } = receiver {
                        types.push(*static_type);
                    }
                },
                out,
            );
            for argument in args {
                collect_expr_type_occurrences(lowerer, argument, out);
            }
        }
        ExprKind::LocalFunctionCall {
            callee,
            captures,
            args,
            ..
        } => {
            push_types_at_expression(
                expression,
                |types| collect_callable_types(lowerer, *callee, types),
                out,
            );
            for value in captures.iter().chain(args) {
                collect_expr_type_occurrences(lowerer, value, out);
            }
        }
        ExprKind::CallableCall { callee, args, .. } => {
            collect_expr_type_occurrences(lowerer, callee, out);
            for argument in args {
                collect_expr_type_occurrences(lowerer, argument, out);
            }
        }
        ExprKind::IntegerOperation { arguments, .. } => match arguments {
            hir::HirIntegerOperationArguments::Unary(operand) => {
                collect_expr_type_occurrences(lowerer, operand, out);
            }
            hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                collect_expr_type_occurrences(lowerer, lhs, out);
                collect_expr_type_occurrences(lowerer, rhs, out);
            }
        },
    }
}

fn push_type_at_expression(ty: hir::TypeId, expression: &hir::Expr, out: &mut Vec<TypeOccurrence>) {
    push_type_at_origin(ty, expression.origin, out);
}

fn push_type_at_origin(
    ty: hir::TypeId,
    origin: hir::ExpressionOrigin,
    out: &mut Vec<TypeOccurrence>,
) {
    let evaluation = origin.concrete().evaluation;
    out.push(TypeOccurrence {
        ty,
        file: evaluation.file as usize,
        span: evaluation.span,
    });
}

fn push_types_at_expression(
    expression: &hir::Expr,
    collect: impl FnOnce(&mut Vec<hir::TypeId>),
    out: &mut Vec<TypeOccurrence>,
) {
    push_types_at_origin(expression.origin, collect, out);
}

fn push_types_at_origin(
    origin: hir::ExpressionOrigin,
    collect: impl FnOnce(&mut Vec<hir::TypeId>),
    out: &mut Vec<TypeOccurrence>,
) {
    let evaluation = origin.concrete().evaluation;
    push_types_at(evaluation.file as usize, evaluation.span, collect, out);
}

fn push_types_at(
    file: usize,
    span: scoop_ast::Span,
    collect: impl FnOnce(&mut Vec<hir::TypeId>),
    out: &mut Vec<TypeOccurrence>,
) {
    let mut types = Vec::new();
    collect(&mut types);
    out.extend(
        types
            .into_iter()
            .map(|ty| TypeOccurrence { ty, file, span }),
    );
}
