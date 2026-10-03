use super::*;

pub(super) fn validate_c_abi_projections(module: &Module) -> Result<(), MirValidationError> {
    for (id, definition) in module.structs.iter() {
        let StructRepresentation::Declared {
            c_layout,
            c_abi: StructCAbi::UInt64Field { field },
            fields,
            ..
        } = &definition.representation
        else {
            continue;
        };
        if c_layout.is_some()
            || !definition.gc_free
            || !matches!(fields.as_slice(), [only] if only.identity == *field && only.ty == Type::Integer(IntegerKind::UNSIGNED_64))
        {
            return Err(MirValidationError {
                location: MirValidationLocation::RuntimeType {
                    location: MirRuntimeTypeLocation::Struct(id),
                },
                kind: MirValidationErrorKind::InvalidStructCAbiProjection,
            });
        }
    }
    Ok(())
}
