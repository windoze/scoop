use super::*;
use hir::ExprKind as E;

pub(super) fn operands(expression: &hir::Expr) -> Vec<&hir::Expr> {
    match &expression.kind {
        E::MaybeUninit(operation) => operation
            .operand()
            .map(|value| value.as_ref())
            .into_iter()
            .collect(),
        E::Atomic(atomic) => atomic.operands().collect(),
        E::IntegerOperation(operation) => {
            operation.operands().map(|value| value.as_ref()).collect()
        }
        E::Binary { lhs, rhs, .. }
        | E::PrimitiveBinary { lhs, rhs, .. }
        | E::FloatBinary { lhs, rhs, .. } => vec![lhs, rhs],
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
        | E::Upcast(operand)
        | E::ArrayLen(operand)
        | E::ArrayClone(operand)
        | E::AtomicNew(operand)
        | E::SomeWrap(operand)
        | E::IsSome(operand) => vec![operand],
        E::FunctionCoercion { source, .. } => vec![source],
        E::FieldAccess { receiver, .. } => vec![receiver],
        E::ForeignCallbackRegister { closure, .. } => vec![closure],
        E::ForeignCallbackOperation { callback, .. } => vec![callback],
        E::TupleLiteral(values)
        | E::ArrayLiteral(values)
        | E::StructConstruct { fields: values, .. }
        | E::StructInit { args: values, .. }
        | E::ClassInit { args: values, .. }
        | E::VariantConstruct { args: values, .. }
        | E::Call { args: values, .. } => values.iter().collect(),
        E::MethodCall { receiver, args, .. }
        | E::DirectSuperMethodCall { receiver, args, .. }
        | E::CallableCall {
            callee: receiver,
            args,
            ..
        } => std::iter::once(receiver.as_ref())
            .chain(args.iter())
            .collect(),
        E::PtrLoad { pointer, offset } => std::iter::once(pointer.as_ref())
            .chain(offset.as_deref())
            .collect(),
        E::PtrStore {
            pointer,
            offset,
            value,
        } => std::iter::once(pointer.as_ref())
            .chain(offset.as_deref())
            .chain([value.as_ref()])
            .collect(),
        E::PtrOffset {
            pointer, offset, ..
        } => vec![pointer, offset],
        E::Index {
            receiver, index, ..
        } => vec![receiver, index],
        E::ArraySet {
            receiver,
            index,
            value,
            ..
        } => vec![receiver, index, value],
        E::ArrayGenerate { count, initializer } => vec![count, initializer],
        E::ArrayAssembly(assembly) => assembly
            .parts
            .iter()
            .map(|part| match part {
                hir::ArrayAssemblyPart::Element(value)
                | hir::ArrayAssemblyPart::CopyArray(value) => value,
            })
            .collect(),
        E::ContextLookup(_)
        | E::StringLiteral { .. }
        | E::ByteArrayLiteral(_)
        | E::EmbeddedFileArrayLiteral(_)
        | E::IntegerLiteral(_)
        | E::CharLiteral(_)
        | E::FloatLiteral(_)
        | E::BoolLiteral(_)
        | E::Unreachable
        | E::AnnotationKey(_)
        | E::UnitLiteral
        | E::Local(_)
        | E::ConstructorReceiver
        | E::ConstructorParam(_)
        | E::InitializingClassFieldAccess { .. }
        | E::ReleaseFieldLoad(_)
        | E::InitializingStructFieldAccess { .. }
        | E::GlobalRead(_)
        | E::GenericDelegateStorageRead(_)
        | E::SingletonValue(_)
        | E::Capture(_)
        | E::NoneLiteral
        | E::AddressOf(_)
        | E::SizeOf(_)
        | E::AlignOf(_)
        | E::FunctionAddress(_)
        | E::Lambda(_)
        | E::AnonymousFunction(_)
        | E::CallableReference(_) => Vec::new(),
    }
}

pub(super) fn may_throw(expression: &hir::Expr) -> bool {
    matches!(
        expression.kind,
        E::Call { .. }
            | E::MethodCall { .. }
            | E::DirectSuperMethodCall { .. }
            | E::CallableCall { .. }
            | E::ClassInit { .. }
            | E::StructInit { .. }
            | E::Box(_)
            | E::ArrayLiteral(_)
            | E::ArrayGenerate { .. }
            | E::ArrayClone(_)
            | E::ArrayAssembly(_)
            | E::Index { .. }
            | E::ArraySet { .. }
            | E::Cast { .. }
            | E::Unwrap {
                trap_on_none: true,
                ..
            }
            | E::PrimitiveBinary { .. }
            | E::IntegerOperation(_)
            | E::Lambda(_)
            | E::AnonymousFunction(_)
            | E::CallableReference(_)
            | E::FunctionCoercion { .. }
            | E::AtomicNew(_)
            | E::ForeignCallbackRegister { .. }
            | E::ForeignCallbackOperation { .. }
    )
}
