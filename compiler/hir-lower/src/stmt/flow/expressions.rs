//! Normal completion follows the operands that actually execute.

use super::*;
use hir::ExprKind as E;

impl Flow<'_> {
    pub(super) fn expression_can_complete(&self, expression: &hir::Expr) -> bool {
        if (self.is_nothing)(expression.ty) {
            return false;
        }
        let complete = |value: &hir::Expr| self.expression_can_complete(value);
        match &expression.kind {
            E::MaybeUninit(operation) => operation.operand().is_none_or(|value| complete(value)),
            E::Atomic(atomic) => atomic.operands().all(complete),
            E::Binary {
                op: hir::BinOp::And | hir::BinOp::Or,
                lhs,
                rhs,
            } => {
                complete(lhs)
                    && match (&expression.kind, &lhs.kind) {
                        (
                            E::Binary {
                                op: hir::BinOp::And,
                                ..
                            },
                            E::BoolLiteral(true),
                        )
                        | (
                            E::Binary {
                                op: hir::BinOp::Or, ..
                            },
                            E::BoolLiteral(false),
                        ) => complete(rhs),
                        _ => true,
                    }
            }
            E::Binary { lhs, rhs, .. }
            | E::PrimitiveBinary { lhs, rhs, .. }
            | E::FloatBinary { lhs, rhs, .. } => complete(lhs) && complete(rhs),
            E::FloatUnary { operand, .. }
            | E::FloatConversion { operand, .. }
            | E::PrimitiveUnary { operand, .. }
            | E::IntegerConversion { operand, .. }
            | E::Unary { operand, .. }
            | E::VariantTest { operand, .. }
            | E::VariantPayloadProject { operand, .. }
            | E::IsInstance { operand, .. }
            | E::Cast { operand, .. }
            | E::Unwrap { operand, .. }
            | E::CharCode(operand)
            | E::CharFromCodeUnchecked(operand)
            | E::PtrFromNonZeroULong(operand)
            | E::PtrToULong(operand)
            | E::PtrCast(operand)
            | E::Box(operand)
            | E::Unbox(operand)
            | E::ReferenceUpcast(operand)
            | E::ArrayLen(operand)
            | E::ArrayClone(operand)
            | E::AtomicNew(operand)
            | E::SomeWrap(operand)
            | E::IsSome(operand) => complete(operand),
            E::FunctionCoercion { source, .. } => complete(source),
            E::FieldAccess { receiver, .. } => complete(receiver),
            E::ForeignCallbackRegister { closure, .. } => complete(closure),
            E::ForeignCallbackOperation { callback, .. } => complete(callback),
            E::TupleLiteral(values)
            | E::ArrayLiteral(values)
            | E::StructConstruct { fields: values, .. }
            | E::StructInit { args: values, .. }
            | E::ClassInit { args: values, .. }
            | E::VariantConstruct { args: values, .. }
            | E::Call { args: values, .. } => values.iter().all(complete),
            E::MethodCall { receiver, args, .. }
            | E::DirectSuperMethodCall { receiver, args, .. }
            | E::CallableCall {
                callee: receiver,
                args,
                ..
            } => complete(receiver) && args.iter().all(complete),
            E::PtrLoad { pointer, offset } => {
                complete(pointer) && offset.as_deref().is_none_or(complete)
            }
            E::PtrStore {
                pointer,
                offset,
                value,
            } => complete(pointer) && offset.as_deref().is_none_or(complete) && complete(value),
            E::PtrOffset {
                pointer, offset, ..
            } => complete(pointer) && complete(offset),
            E::Index {
                receiver, index, ..
            } => complete(receiver) && complete(index),
            E::ArraySet {
                receiver,
                index,
                value,
                ..
            } => complete(receiver) && complete(index) && complete(value),
            E::ArrayGenerate { count, initializer } => complete(count) && complete(initializer),
            E::ArrayAssembly(assembly) => assembly.parts.iter().all(|part| match part {
                hir::ArrayAssemblyPart::Element(value)
                | hir::ArrayAssemblyPart::CopyArray(value) => complete(value),
            }),
            E::IntegerOperation { arguments, .. } => match arguments {
                hir::HirIntegerOperationArguments::Unary(operand) => complete(operand),
                hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                    complete(lhs) && complete(rhs)
                }
            },
            E::ContextLookup(_)
            | E::StringLiteral { .. }
            | E::IntegerLiteral(_)
            | E::BoolLiteral(_)
            | E::CharLiteral(_)
            | E::FloatLiteral(_)
            | E::UnitLiteral
            | E::ConstructorParam(_)
            | E::ConstructorReceiver
            | E::Local(_)
            | E::GlobalRead(_)
            | E::GenericDelegateStorageRead(_)
            | E::SingletonValue(_)
            | E::Capture(_)
            | E::Lambda(_)
            | E::AnonymousFunction(_)
            | E::CallableReference(_)
            | E::AddressOf(_)
            | E::SizeOf(_)
            | E::AlignOf(_)
            | E::FunctionAddress(_)
            | E::InitializingClassFieldAccess { .. }
            | E::ReleaseFieldLoad(_)
            | E::InitializingStructFieldAccess { .. }
            | E::NoneLiteral => true,
        }
    }
}
