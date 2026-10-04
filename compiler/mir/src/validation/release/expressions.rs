use super::*;

pub(in crate::validation) fn validate_expression(
    module: &Module,
    owner: LocalValueOwner,
    expression: &Expr,
) -> Result<(), MirValidationErrorKind> {
    let LocalValueOwner::ReleaseHook(hook) = owner else {
        return if matches!(expression.kind, ExprKind::ReleaseFieldLoad { .. }) {
            Err(invalid("release field read requires a release body"))
        } else {
            Ok(())
        };
    };
    if !gc_free(module, &expression.ty) {
        return Err(invalid("release expression cannot carry a managed value"));
    }
    match &expression.kind {
        ExprKind::ReleaseFieldLoad { class, index } => {
            if *class != module.release_hooks[hook].owner {
                return Err(invalid("release field read must use its own exact owner"));
            }
            let ClassRepresentation::Declared { fields, base_class } =
                &module.classes[*class].representation
            else {
                return Err(invalid("release field read requires a declared class"));
            };
            let first = base_class.map_or(0, |base| module.classes[base].declared_fields().len());
            if (*index as usize) < first
                || fields
                    .get(*index as usize)
                    .is_none_or(|field| field.ty != expression.ty)
            {
                return Err(invalid(
                    "release field read must name an own field with the exact result type",
                ));
            }
            Ok(())
        }
        ExprKind::GlobalRead(global) | ExprKind::GlobalAddress { global, .. } => {
            check_global(module, *global)
        }
        ExprKind::IntegerLiteral(_)
        | ExprKind::MachineScalarLiteral(_)
        | ExprKind::CharLiteral(_)
        | ExprKind::BoolLiteral(_)
        | ExprKind::UnitLiteral
        | ExprKind::TupleLiteral(_)
        | ExprKind::StructInit { .. }
        | ExprKind::StructConstruct { .. }
        | ExprKind::Local(_)
        | ExprKind::PtrFromNonZeroULong { .. }
        | ExprKind::CharCode(_)
        | ExprKind::CharFromCodeUnchecked(_)
        | ExprKind::PtrToULong(_)
        | ExprKind::PtrCast { .. }
        | ExprKind::PtrLoad { .. }
        | ExprKind::PtrStore { .. }
        | ExprKind::PtrOffset { .. }
        | ExprKind::AddressOf { .. }
        | ExprKind::SizeOf(_)
        | ExprKind::AlignOf(_)
        | ExprKind::Retype { .. }
        | ExprKind::FieldAccess { .. }
        | ExprKind::Binary { .. }
        | ExprKind::Unary { .. }
        | ExprKind::IntegerUnary { .. }
        | ExprKind::IntegerBinary { .. }
        | ExprKind::IntegerCompare { .. }
        | ExprKind::IntegerCompareTo { .. }
        | ExprKind::IntegerShift { .. }
        | ExprKind::IntegerConversion { .. }
        | ExprKind::VariantConstruct { .. }
        | ExprKind::EnumTag(_)
        | ExprKind::EnumField { .. }
        | ExprKind::VariantTest { .. }
        | ExprKind::VariantPayloadProject { .. } => Ok(()),
        ExprKind::StringConst(_)
        | ExprKind::ClassAlloc { .. }
        | ExprKind::ClosureAlloc { .. }
        | ExprKind::ClosureCapture { .. }
        | ExprKind::InitializationUnitAddress(_)
        | ExprKind::FunctionAddress { .. }
        | ExprKind::ForeignCallbackRegister { .. }
        | ExprKind::ForeignCallbackOperation { .. }
        | ExprKind::CaughtException
        | ExprKind::AtomicFieldLoad { .. }
        | ExprKind::AtomicFieldCompareExchange { .. }
        | ExprKind::Box(_)
        | ExprKind::Unbox(_)
        | ExprKind::IsInstance { .. }
        | ExprKind::Cast { .. }
        | ExprKind::ArrayAllocate { .. }
        | ExprKind::ArrayLiteral { .. }
        | ExprKind::ArrayAssembly { .. }
        | ExprKind::ArrayGet { .. }
        | ExprKind::ArrayLen { .. }
        | ExprKind::ArrayClone { .. }
        | ExprKind::SafeIntegerDivRem { .. } => Err(invalid(
            "operation is outside the release body's GC-free subset",
        )),
    }
}
