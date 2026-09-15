use std::collections::HashSet;

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum CallableValue {
    Lambda(u32),
    AnonymousFunction(u32),
    CallableReference(u32),
    LocalFunction(u32),
}

enum LocalQuery<'a> {
    Outside {
        known: &'a HashSet<u32>,
        allowed: &'a HashSet<u32>,
    },
}

pub(super) fn expression_references_outside(
    module: &Module,
    expression: &Expr,
    known: &HashSet<u32>,
    allowed: &HashSet<u32>,
) -> bool {
    expression_references_any_inner(
        module,
        expression,
        &LocalQuery::Outside { known, allowed },
        &mut HashSet::new(),
    )
}

pub(super) fn pattern_references_outside(
    module: &Module,
    pattern: &Pattern,
    known: &HashSet<u32>,
    allowed: &HashSet<u32>,
) -> bool {
    pattern_references_any(
        module,
        pattern,
        &LocalQuery::Outside { known, allowed },
        &mut HashSet::new(),
    )
}

pub(super) fn assign_target_references_outside(
    module: &Module,
    target: &AssignTarget,
    known: &HashSet<u32>,
    allowed: &HashSet<u32>,
) -> bool {
    assign_target_references_any(
        module,
        target,
        &LocalQuery::Outside { known, allowed },
        &mut HashSet::new(),
    )
}

fn assign_target_references_any(
    module: &Module,
    target: &AssignTarget,
    locals: &LocalQuery<'_>,
    visiting: &mut HashSet<CallableValue>,
) -> bool {
    match target {
        AssignTarget::Local(local) => contains(locals, *local),
        AssignTarget::Index { array, index } => {
            expression_references_any_inner(module, array, locals, visiting)
                || expression_references_any_inner(module, index, locals, visiting)
        }
        AssignTarget::Field { receiver, .. } => {
            expression_references_any_inner(module, receiver, locals, visiting)
        }
        AssignTarget::Global(_)
        | AssignTarget::SingletonPublishedRoot(_)
        | AssignTarget::InitializingClassField { .. } => false,
    }
}

fn pattern_references_any(
    module: &Module,
    pattern: &Pattern,
    locals: &LocalQuery<'_>,
    visiting: &mut HashSet<CallableValue>,
) -> bool {
    match pattern {
        Pattern::Literal { value, .. } => {
            expression_references_any_inner(module, value, locals, visiting)
        }
        Pattern::Variant { fields, .. } | Pattern::Struct { fields, .. } => fields
            .iter()
            .any(|(_, pattern)| pattern_references_any(module, pattern, locals, visiting)),
        Pattern::Tuple(elements) => elements
            .iter()
            .any(|pattern| pattern_references_any(module, pattern, locals, visiting)),
        Pattern::Binding { .. } | Pattern::Wildcard => false,
    }
}

fn expression_references_any_inner(
    module: &Module,
    expression: &Expr,
    locals: &LocalQuery<'_>,
    visiting: &mut HashSet<CallableValue>,
) -> bool {
    match &expression.kind {
        ExprKind::Local(local) => contains(locals, *local),
        ExprKind::TupleLiteral(values)
        | ExprKind::ArrayLiteral(values)
        | ExprKind::StructInit { args: values, .. }
        | ExprKind::StructConstruct { fields: values, .. }
        | ExprKind::ClassInit { args: values, .. }
        | ExprKind::VariantConstruct { args: values, .. }
        | ExprKind::Call { args: values, .. }
        | ExprKind::ImportedCoreCall { args: values, .. } => values
            .iter()
            .any(|value| expression_references_any_inner(module, value, locals, visiting)),
        ExprKind::VariantTest { operand, .. }
        | ExprKind::VariantPayloadProject { operand, .. }
        | ExprKind::FunctionCoercion {
            source: operand, ..
        }
        | ExprKind::PtrFromNonZeroULong(operand)
        | ExprKind::PtrToULong(operand)
        | ExprKind::PtrCast(operand)
        | ExprKind::ForeignCallbackRegister {
            closure: operand, ..
        }
        | ExprKind::ForeignCallbackOperation {
            callback: operand, ..
        }
        | ExprKind::FieldAccess {
            receiver: operand, ..
        }
        | ExprKind::Box(operand)
        | ExprKind::Unbox(operand)
        | ExprKind::IsInstance { operand, .. }
        | ExprKind::Cast { operand, .. }
        | ExprKind::PrimitiveUnary { operand, .. }
        | ExprKind::IntegerConversion { operand, .. }
        | ExprKind::Unary { operand, .. }
        | ExprKind::SomeWrap(operand)
        | ExprKind::IsSome(operand)
        | ExprKind::Unwrap { operand, .. }
        | ExprKind::ArrayLen(operand)
        | ExprKind::ArrayClone(operand) => {
            expression_references_any_inner(module, operand, locals, visiting)
        }
        ExprKind::PtrLoad { pointer, offset } => {
            expression_references_any_inner(module, pointer, locals, visiting)
                || offset.as_deref().is_some_and(|offset| {
                    expression_references_any_inner(module, offset, locals, visiting)
                })
        }
        ExprKind::PtrStore {
            pointer,
            offset,
            value,
        } => {
            expression_references_any_inner(module, pointer, locals, visiting)
                || offset.as_deref().is_some_and(|offset| {
                    expression_references_any_inner(module, offset, locals, visiting)
                })
                || expression_references_any_inner(module, value, locals, visiting)
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
            expression_references_any_inner(module, pointer, locals, visiting)
                || expression_references_any_inner(module, offset, locals, visiting)
        }
        ExprKind::AddressOf(Place::Local(local)) => contains(locals, *local),
        ExprKind::AddressOf(Place::Global(_)) => false,
        ExprKind::MethodCall { receiver, args, .. }
        | ExprKind::DirectSuperMethodCall { receiver, args, .. } => {
            expression_references_any_inner(module, receiver, locals, visiting)
                || args.iter().any(|argument| {
                    expression_references_any_inner(module, argument, locals, visiting)
                })
        }
        ExprKind::ArrayAssembly(assembly) => assembly.parts.iter().any(|part| match part {
            ArrayAssemblyPart::Element(value) | ArrayAssemblyPart::CopyArray(value) => {
                expression_references_any_inner(module, value, locals, visiting)
            }
        }),
        ExprKind::ArraySet {
            receiver,
            index,
            value,
            ..
        } => {
            expression_references_any_inner(module, receiver, locals, visiting)
                || expression_references_any_inner(module, index, locals, visiting)
                || expression_references_any_inner(module, value, locals, visiting)
        }
        ExprKind::LocalFunctionCall { captures, args, .. } => captures
            .iter()
            .chain(args)
            .any(|value| expression_references_any_inner(module, value, locals, visiting)),
        ExprKind::CallableCall { callee, args, .. } => {
            expression_references_any_inner(module, callee, locals, visiting)
                || args.iter().any(|argument| {
                    expression_references_any_inner(module, argument, locals, visiting)
                })
        }
        ExprKind::IntegerOperation { arguments, .. } => match arguments {
            HirIntegerOperationArguments::Unary(operand) => {
                expression_references_any_inner(module, operand, locals, visiting)
            }
            HirIntegerOperationArguments::Binary { lhs, rhs } => {
                expression_references_any_inner(module, lhs, locals, visiting)
                    || expression_references_any_inner(module, rhs, locals, visiting)
            }
        },
        ExprKind::Lambda(id) => callable_value_references_any(
            CallableValue::Lambda(id.into_raw().into_u32()),
            visiting,
            |visiting| {
                let Some(lambda) = checked_arena(&module.lambdas, *id) else {
                    return true;
                };
                captures_reference_any(module, &lambda.captures, locals, visiting)
            },
        ),
        ExprKind::AnonymousFunction(id) => callable_value_references_any(
            CallableValue::AnonymousFunction(id.into_raw().into_u32()),
            visiting,
            |visiting| {
                let Some(function) = checked_arena(&module.anonymous_functions, *id) else {
                    return true;
                };
                captures_reference_any(module, &function.captures, locals, visiting)
            },
        ),
        ExprKind::CallableReference(id) => callable_value_references_any(
            CallableValue::CallableReference(id.into_raw().into_u32()),
            visiting,
            |visiting| {
                let Some(reference) = checked_arena(&module.callable_references, *id) else {
                    return true;
                };
                captures_reference_any(module, &reference.captures, locals, visiting)
                    || match &reference.target {
                        CallableReferenceTarget::BoundMember { receiver, .. }
                        | CallableReferenceTarget::BoundExtension { receiver, .. } => {
                            expression_references_any_inner(module, receiver, locals, visiting)
                        }
                        CallableReferenceTarget::Local { local_function, .. } => {
                            callable_value_references_any(
                                CallableValue::LocalFunction(local_function.into_raw().into_u32()),
                                visiting,
                                |visiting| {
                                    let Some(function) =
                                        checked_arena(&module.local_functions, *local_function)
                                    else {
                                        return true;
                                    };
                                    captures_reference_any(
                                        module,
                                        &function.captures,
                                        locals,
                                        visiting,
                                    )
                                },
                            )
                        }
                        CallableReferenceTarget::Named(_) => false,
                    }
            },
        ),
        ExprKind::StringLiteral { .. }
        | ExprKind::IntegerLiteral(_)
        | ExprKind::BoolLiteral(_)
        | ExprKind::UnitLiteral
        | ExprKind::ConstructorParam(_)
        | ExprKind::GlobalRead(_)
        | ExprKind::SingletonValue(_)
        | ExprKind::Capture(_)
        | ExprKind::SizeOf(_)
        | ExprKind::AlignOf(_)
        | ExprKind::FunctionAddress(_)
        | ExprKind::InitializingClassFieldAccess { .. }
        | ExprKind::InitializingStructFieldAccess { .. }
        | ExprKind::NoneLiteral => false,
    }
}

fn captures_reference_any(
    module: &Module,
    captures: &[Capture],
    locals: &LocalQuery<'_>,
    visiting: &mut HashSet<CallableValue>,
) -> bool {
    captures
        .iter()
        .any(|capture| expression_references_any_inner(module, &capture.source, locals, visiting))
}

fn callable_value_references_any(
    value: CallableValue,
    visiting: &mut HashSet<CallableValue>,
    check: impl FnOnce(&mut HashSet<CallableValue>) -> bool,
) -> bool {
    if !visiting.insert(value) {
        return true;
    }
    let references_any = check(visiting);
    visiting.remove(&value);
    references_any
}

fn contains(locals: &LocalQuery<'_>, local: LocalId) -> bool {
    let local = local.into_raw().into_u32();
    match locals {
        LocalQuery::Outside { known, allowed } => {
            !known.contains(&local) || !allowed.contains(&local)
        }
    }
}
