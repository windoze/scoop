use super::*;

/// Recursive calls may be lowered before a later source use discovers the
/// complete capture set. Once the local body has been analyzed, rewrite every
/// self-call in that body to the final hidden-argument list. Binding identity,
/// rather than source names, makes this stable under shadowing.
pub(super) fn patch_local_function_calls(
    statements: &mut [hir::Statement],
    target: hir::LocalFunctionId,
    captures: &[hir::Capture],
) {
    for statement in statements {
        match &mut statement.kind {
            hir::StatementKind::InitializationEnsure(_) => {}
            hir::StatementKind::Expr(expr) | hir::StatementKind::Throw(expr) => {
                patch_local_function_call_expr(expr, target, captures)
            }
            hir::StatementKind::Return { value } => {
                if let Some(value) = value {
                    patch_local_function_call_expr(value, target, captures);
                }
            }
            hir::StatementKind::LocalFunction(_) => {}
            hir::StatementKind::ValDecl { pattern, init } => {
                patch_local_function_call_pattern(pattern, target, captures);
                patch_local_function_call_expr(init, target, captures);
            }
            hir::StatementKind::Assign {
                target: place,
                value,
            } => {
                match place {
                    hir::AssignTarget::Local(_)
                    | hir::AssignTarget::Global(_)
                    | hir::AssignTarget::SingletonPublishedRoot(_) => {}
                    hir::AssignTarget::Index { array, index } => {
                        patch_local_function_call_expr(array, target, captures);
                        patch_local_function_call_expr(index, target, captures);
                    }
                    hir::AssignTarget::Field { receiver, .. } => {
                        patch_local_function_call_expr(receiver, target, captures);
                    }
                    hir::AssignTarget::InitializingClassField { .. } => {}
                }
                patch_local_function_call_expr(value, target, captures);
            }
            hir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => {
                patch_local_function_call_expr(cond, target, captures);
                patch_local_function_calls(then_body, target, captures);
                if let Some(else_body) = else_body {
                    patch_local_function_calls(else_body, target, captures);
                }
            }
            hir::StatementKind::While {
                condition_setup,
                cond,
                body,
            } => {
                patch_local_function_calls(condition_setup, target, captures);
                patch_local_function_call_expr(cond, target, captures);
                patch_local_function_calls(body, target, captures);
            }
            hir::StatementKind::When(when) => {
                patch_local_function_call_expr(&mut when.subject, target, captures);
                for arm in &mut when.arms {
                    patch_local_function_call_pattern(&mut arm.pattern, target, captures);
                    if let Some(guard) = &mut arm.guard {
                        patch_local_function_calls(&mut guard.setup, target, captures);
                        patch_local_function_call_expr(&mut guard.condition, target, captures);
                    }
                    patch_local_function_calls(&mut arm.body, target, captures);
                }
                if let hir::WhenFallback::Else(body) = &mut when.fallback {
                    patch_local_function_calls(body, target, captures);
                }
            }
            hir::StatementKind::Try(try_) => {
                patch_local_function_calls(&mut try_.body, target, captures);
                for catch in &mut try_.catches {
                    patch_local_function_calls(&mut catch.body, target, captures);
                }
                if let Some(finally_body) = &mut try_.finally_body {
                    patch_local_function_calls(finally_body, target, captures);
                }
            }
        }
    }
}

fn patch_local_function_call_pattern(
    pattern: &mut hir::Pattern,
    target: hir::LocalFunctionId,
    captures: &[hir::Capture],
) {
    match pattern {
        hir::Pattern::Literal { value, .. } => {
            patch_local_function_call_expr(value, target, captures)
        }
        hir::Pattern::Variant { fields, .. } | hir::Pattern::Struct { fields, .. } => {
            for (_, field) in fields {
                patch_local_function_call_pattern(field, target, captures);
            }
        }
        hir::Pattern::Tuple(elements) => {
            for element in elements {
                patch_local_function_call_pattern(element, target, captures);
            }
        }
        hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => {}
    }
}

fn patch_local_function_call_expr(
    expr: &mut hir::Expr,
    target: hir::LocalFunctionId,
    target_captures: &[hir::Capture],
) {
    let span = expr.span;
    let origin = expr.origin;
    match &mut expr.kind {
        hir::ExprKind::LocalFunctionCall {
            local_function,
            captures,
            args,
            ..
        } => {
            for capture in captures.iter_mut() {
                patch_local_function_call_expr(capture, target, target_captures);
            }
            for arg in args {
                patch_local_function_call_expr(arg, target, target_captures);
            }
            if *local_function == target && captures.len() != target_captures.len() {
                *captures = target_captures
                    .iter()
                    .map(|capture| hir::Expr {
                        kind: hir::ExprKind::Capture(capture.binding),
                        ty: capture.ty,
                        span,
                        origin,
                    })
                    .collect();
            }
        }
        hir::ExprKind::TupleLiteral(elements)
        | hir::ExprKind::ArrayLiteral(elements)
        | hir::ExprKind::StructInit { args: elements, .. }
        | hir::ExprKind::ClassInit { args: elements, .. }
        | hir::ExprKind::VariantConstruct { args: elements, .. }
        | hir::ExprKind::Call { args: elements, .. } => {
            for element in elements {
                patch_local_function_call_expr(element, target, target_captures);
            }
        }
        hir::ExprKind::ArrayAssembly(assembly) => {
            for part in &mut assembly.parts {
                match part {
                    hir::ArrayAssemblyPart::Element(value)
                    | hir::ArrayAssemblyPart::CopyArray(value) => {
                        patch_local_function_call_expr(value, target, target_captures)
                    }
                }
            }
        }
        hir::ExprKind::FieldAccess { receiver, .. }
        | hir::ExprKind::ForeignCallbackRegister {
            closure: receiver, ..
        }
        | hir::ExprKind::ForeignCallbackOperation {
            callback: receiver, ..
        }
        | hir::ExprKind::FunctionCoercion {
            source: receiver, ..
        }
        | hir::ExprKind::Box(receiver)
        | hir::ExprKind::Unbox(receiver)
        | hir::ExprKind::IsInstance {
            operand: receiver, ..
        }
        | hir::ExprKind::Cast {
            operand: receiver, ..
        }
        | hir::ExprKind::ArrayLen(receiver)
        | hir::ExprKind::ArrayClone(receiver)
        | hir::ExprKind::Unary {
            operand: receiver, ..
        }
        | hir::ExprKind::PrimitiveUnary {
            operand: receiver, ..
        }
        | hir::ExprKind::SomeWrap(receiver)
        | hir::ExprKind::IsSome(receiver)
        | hir::ExprKind::Unwrap {
            operand: receiver, ..
        }
        | hir::ExprKind::PtrFromNonZeroULong(receiver)
        | hir::ExprKind::PtrToULong(receiver)
        | hir::ExprKind::PtrCast(receiver) => {
            patch_local_function_call_expr(receiver, target, target_captures)
        }
        hir::ExprKind::MethodCall { receiver, args, .. }
        | hir::ExprKind::DirectSuperMethodCall { receiver, args, .. }
        | hir::ExprKind::CallableCall {
            callee: receiver,
            args,
            ..
        } => {
            patch_local_function_call_expr(receiver, target, target_captures);
            for arg in args {
                patch_local_function_call_expr(arg, target, target_captures);
            }
        }
        hir::ExprKind::Index {
            receiver, index, ..
        } => {
            patch_local_function_call_expr(receiver, target, target_captures);
            patch_local_function_call_expr(index, target, target_captures);
        }
        hir::ExprKind::PrimitiveBinary { lhs, rhs, .. }
        | hir::ExprKind::Binary { lhs, rhs, .. } => {
            patch_local_function_call_expr(lhs, target, target_captures);
            patch_local_function_call_expr(rhs, target, target_captures);
        }
        hir::ExprKind::IntegerOperation { arguments, .. } => match arguments {
            hir::HirIntegerOperationArguments::Unary(operand) => {
                patch_local_function_call_expr(operand, target, target_captures);
            }
            hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                patch_local_function_call_expr(lhs, target, target_captures);
                patch_local_function_call_expr(rhs, target, target_captures);
            }
        },
        hir::ExprKind::IntegerConversion { operand, .. } => {
            patch_local_function_call_expr(operand, target, target_captures);
        }
        hir::ExprKind::ArraySet {
            receiver,
            index,
            value,
            ..
        } => {
            patch_local_function_call_expr(receiver, target, target_captures);
            patch_local_function_call_expr(index, target, target_captures);
            patch_local_function_call_expr(value, target, target_captures);
        }
        hir::ExprKind::PtrLoad { pointer, offset } => {
            patch_local_function_call_expr(pointer, target, target_captures);
            if let Some(offset) = offset {
                patch_local_function_call_expr(offset, target, target_captures);
            }
        }
        hir::ExprKind::PtrOffset {
            pointer, offset, ..
        } => {
            patch_local_function_call_expr(pointer, target, target_captures);
            patch_local_function_call_expr(offset, target, target_captures);
        }
        hir::ExprKind::PtrStore {
            pointer,
            offset,
            value,
        } => {
            patch_local_function_call_expr(pointer, target, target_captures);
            if let Some(offset) = offset {
                patch_local_function_call_expr(offset, target, target_captures);
            }
            patch_local_function_call_expr(value, target, target_captures);
        }
        hir::ExprKind::StringLiteral(_)
        | hir::ExprKind::IntegerLiteral(_)
        | hir::ExprKind::BoolLiteral(_)
        | hir::ExprKind::UnitLiteral
        | hir::ExprKind::Local(_)
        | hir::ExprKind::ConstructorParam(_)
        | hir::ExprKind::InitializingClassFieldAccess { .. }
        | hir::ExprKind::InitializingStructFieldAccess { .. }
        | hir::ExprKind::GlobalRead(_)
        | hir::ExprKind::SingletonValue(_)
        | hir::ExprKind::Capture(_)
        | hir::ExprKind::Lambda(_)
        | hir::ExprKind::AnonymousFunction(_)
        | hir::ExprKind::CallableReference(_)
        | hir::ExprKind::NoneLiteral
        | hir::ExprKind::AddressOf(_)
        | hir::ExprKind::SizeOf(_)
        | hir::ExprKind::AlignOf(_)
        | hir::ExprKind::FunctionAddress(_) => {}
    }
}
