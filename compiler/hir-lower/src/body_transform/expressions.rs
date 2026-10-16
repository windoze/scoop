use super::*;

pub(crate) fn walk(
    visitor: &mut (impl BodyTransform + ?Sized),
    lowerer: &mut Lowerer,
    expr: &mut hir::Expr,
) {
    match &mut expr.kind {
        hir::ExprKind::MaybeUninit(operation) => {
            if let Some(operand) = operation.operand_mut() {
                visitor.expression(lowerer, operand);
            }
        }
        hir::ExprKind::Atomic(atomic) => {
            for operand in atomic.operands_mut() {
                visitor.expression(lowerer, operand);
            }
        }
        hir::ExprKind::Lambda(id) => {
            let mut closure = lowerer.lambdas[*id].clone();
            for capture in &mut closure.captures {
                visitor.expression(lowerer, &mut capture.source);
            }
            lowerer.lambdas[*id] = closure;
        }
        hir::ExprKind::AnonymousFunction(id) => {
            let mut closure = lowerer.anonymous_functions[*id].clone();
            for capture in &mut closure.captures {
                visitor.expression(lowerer, &mut capture.source);
            }
            lowerer.anonymous_functions[*id] = closure;
        }
        hir::ExprKind::CallableReference(id) => {
            let mut reference = lowerer.callable_references[*id].clone();
            if let Some(receiver) = reference.target.receiver_mut() {
                visitor.expression(lowerer, receiver);
            }
            for capture in &mut reference.captures {
                visitor.expression(lowerer, &mut capture.source);
            }
            lowerer.callable_references[*id] = reference;
        }
        hir::ExprKind::Call { args, .. } => {
            for argument in args.iter_mut() {
                visitor.expression(lowerer, argument);
            }
        }

        hir::ExprKind::TupleLiteral(elements)
        | hir::ExprKind::ArrayLiteral(elements)
        | hir::ExprKind::StructInit { args: elements, .. }
        | hir::ExprKind::ClassInit { args: elements, .. }
        | hir::ExprKind::VariantConstruct { args: elements, .. } => {
            for element in elements {
                visitor.expression(lowerer, element);
            }
        }
        hir::ExprKind::StructConstruct {
            fields: elements, ..
        } => {
            for element in elements {
                visitor.expression(lowerer, element);
            }
        }
        hir::ExprKind::ArrayAssembly(assembly) => {
            for part in &mut assembly.parts {
                match part {
                    hir::ArrayAssemblyPart::Element(value)
                    | hir::ArrayAssemblyPart::CopyArray(value) => {
                        visitor.expression(lowerer, value)
                    }
                }
            }
        }
        hir::ExprKind::FieldAccess { receiver, .. }
        | hir::ExprKind::VariantTest {
            operand: receiver, ..
        }
        | hir::ExprKind::VariantPayloadProject {
            operand: receiver, ..
        }
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
        | hir::ExprKind::Upcast(receiver)
        | hir::ExprKind::IsInstance {
            operand: receiver, ..
        }
        | hir::ExprKind::Cast {
            operand: receiver, ..
        }
        | hir::ExprKind::ArrayLen(receiver)
        | hir::ExprKind::ArrayClone(receiver)
        | hir::ExprKind::AtomicNew(receiver)
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
        | hir::ExprKind::CharCode(receiver)
        | hir::ExprKind::CharFromCodeUnchecked(receiver)
        | hir::ExprKind::PtrToULong(receiver)
        | hir::ExprKind::PtrCast(receiver) => visitor.expression(lowerer, receiver),
        hir::ExprKind::MethodCall { receiver, args, .. }
        | hir::ExprKind::DirectSuperMethodCall { receiver, args, .. }
        | hir::ExprKind::CallableCall {
            callee: receiver,
            args,
            ..
        } => {
            visitor.expression(lowerer, receiver);
            for arg in args {
                visitor.expression(lowerer, arg);
            }
        }
        hir::ExprKind::Index {
            receiver, index, ..
        } => {
            visitor.expression(lowerer, receiver);
            visitor.expression(lowerer, index);
        }
        hir::ExprKind::PrimitiveBinary { lhs, rhs, .. }
        | hir::ExprKind::ArrayGenerate {
            count: lhs,
            initializer: rhs,
        }
        | hir::ExprKind::Binary { lhs, rhs, .. }
        | hir::ExprKind::FloatBinary { lhs, rhs, .. } => {
            visitor.expression(lowerer, lhs);
            visitor.expression(lowerer, rhs);
        }
        hir::ExprKind::IntegerOperation(operation) => {
            for operand in operation.operands_mut() {
                visitor.expression(lowerer, operand);
            }
        }
        hir::ExprKind::IntegerConversion { operand, .. }
        | hir::ExprKind::FloatUnary { operand, .. }
        | hir::ExprKind::FloatConversion { operand, .. } => {
            visitor.expression(lowerer, operand);
        }
        hir::ExprKind::ArraySet {
            receiver,
            index,
            value,
            ..
        } => {
            visitor.expression(lowerer, receiver);
            visitor.expression(lowerer, index);
            visitor.expression(lowerer, value);
        }
        hir::ExprKind::PtrLoad { pointer, offset } => {
            visitor.expression(lowerer, pointer);
            if let Some(offset) = offset {
                visitor.expression(lowerer, offset);
            }
        }
        hir::ExprKind::PtrOffset {
            pointer, offset, ..
        } => {
            visitor.expression(lowerer, pointer);
            visitor.expression(lowerer, offset);
        }
        hir::ExprKind::PtrStore {
            pointer,
            offset,
            value,
        } => {
            visitor.expression(lowerer, pointer);
            if let Some(offset) = offset {
                visitor.expression(lowerer, offset);
            }
            visitor.expression(lowerer, value);
        }
        hir::ExprKind::ContextLookup(_)
        | hir::ExprKind::StringLiteral { .. }
        | hir::ExprKind::IntegerLiteral(_)
        | hir::ExprKind::CharLiteral(_)
        | hir::ExprKind::FloatLiteral(_)
        | hir::ExprKind::BoolLiteral(_)
        | hir::ExprKind::Unreachable
        | hir::ExprKind::AnnotationKey(_)
        | hir::ExprKind::UnitLiteral
        | hir::ExprKind::Local(_)
        | hir::ExprKind::ConstructorReceiver
        | hir::ExprKind::ConstructorParam(_)
        | hir::ExprKind::InitializingClassFieldAccess { .. }
        | hir::ExprKind::ReleaseFieldLoad(_)
        | hir::ExprKind::InitializingStructFieldAccess { .. }
        | hir::ExprKind::GlobalRead(_)
        | hir::ExprKind::GenericDelegateStorageRead(_)
        | hir::ExprKind::SingletonValue(_)
        | hir::ExprKind::Capture(_)
        | hir::ExprKind::NoneLiteral
        | hir::ExprKind::AddressOf(_)
        | hir::ExprKind::SizeOf(_)
        | hir::ExprKind::AlignOf(_)
        | hir::ExprKind::FunctionAddress(_) => {}
    }
}
