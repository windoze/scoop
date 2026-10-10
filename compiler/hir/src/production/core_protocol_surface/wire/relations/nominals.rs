use super::*;

pub(super) fn validate_nominal_relations(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    if surface.fundamental_types.unit != CoreBuiltinNominal::Unit.identity_record().id() {
        return Err(
            CoreCompilerProtocolSurfaceValidationError::NominalRoleMismatch {
                product: CoreProtocolProductKindV1::Fundamental,
                index: 0,
            },
        );
    }
    for (index, id) in [
        (0, surface.fundamental_types.unit),
        (1, surface.fundamental_types.int8),
        (2, surface.fundamental_types.int16),
        (3, surface.fundamental_types.int32),
        (4, surface.fundamental_types.int64),
        (5, surface.fundamental_types.uint8),
        (6, surface.fundamental_types.uint16),
        (7, surface.fundamental_types.uint32),
        (8, surface.fundamental_types.uint64),
        (9, surface.fundamental_types.boolean),
    ] {
        validate_nominal_shape(
            CoreProtocolProductKindV1::Fundamental,
            CoreProtocolNominalV1::Type(id),
            index,
            SourceDeclarationKind::Struct,
            0,
            foundation,
        )?;
    }
    validate_nominal_shape(
        CoreProtocolProductKindV1::Fundamental,
        CoreProtocolNominalV1::Type(surface.fundamental_types.string),
        10,
        SourceDeclarationKind::Class,
        0,
        foundation,
    )?;
    for (index, id) in [
        (11, surface.fundamental_types.array),
        (12, surface.fundamental_types.mutable_array),
    ] {
        validate_nominal_shape(
            CoreProtocolProductKindV1::Fundamental,
            CoreProtocolNominalV1::GenericType(id),
            index,
            SourceDeclarationKind::Class,
            1,
            foundation,
        )?;
    }
    for (index, id) in [
        (13, surface.fundamental_types.ptr),
        (14, surface.fundamental_types.fun_ptr),
    ] {
        validate_nominal_shape(
            CoreProtocolProductKindV1::Fundamental,
            CoreProtocolNominalV1::GenericType(id),
            index,
            SourceDeclarationKind::Struct,
            1,
            foundation,
        )?;
    }
    validate_nominal_shape(
        CoreProtocolProductKindV1::Option,
        CoreProtocolNominalV1::GenericType(surface.option_protocol.option),
        0,
        SourceDeclarationKind::Enum,
        1,
        foundation,
    )?;
    validate_nominal_shape(
        CoreProtocolProductKindV1::Iteration,
        CoreProtocolNominalV1::GenericType(surface.iteration_protocol.iterator),
        0,
        SourceDeclarationKind::Interface,
        1,
        foundation,
    )?;
    for (index, id) in [
        (0, surface.exception_protocol.throwable),
        (2, surface.exception_protocol.unwrap_exception),
        (4, surface.exception_protocol.class_cast_exception),
        (6, surface.exception_protocol.arithmetic_exception),
        (8, surface.exception_protocol.index_out_of_bounds_exception),
        (10, surface.exception_protocol.illegal_state_exception),
        (13, surface.exception_protocol.illegal_argument_exception),
    ] {
        validate_nominal_shape(
            CoreProtocolProductKindV1::Exception,
            CoreProtocolNominalV1::Type(id),
            index,
            SourceDeclarationKind::Class,
            0,
            foundation,
        )?;
    }
    for (index, id) in [
        (0, surface.coroutine_protocol.continuation),
        (5, surface.coroutine_protocol.suspend_task),
        (8, surface.coroutine_protocol.suspend_registration),
    ] {
        validate_nominal_shape(
            CoreProtocolProductKindV1::Coroutine,
            CoreProtocolNominalV1::GenericType(id),
            index,
            SourceDeclarationKind::Interface,
            1,
            foundation,
        )?;
    }
    for (index, id) in [
        (0, surface.ffi_protocol.ptr),
        (1, surface.ffi_protocol.fun_ptr),
        (2, surface.ffi_protocol.pinned_ptr),
        (3, surface.ffi_protocol.gc_handle),
    ] {
        validate_nominal_shape(
            CoreProtocolProductKindV1::Ffi,
            CoreProtocolNominalV1::GenericType(id),
            index,
            SourceDeclarationKind::Struct,
            1,
            foundation,
        )?;
    }
    validate_nominal_shape(
        CoreProtocolProductKindV1::ForeignCallback,
        CoreProtocolNominalV1::GenericType(surface.foreign_callback_protocol.callback),
        0,
        SourceDeclarationKind::Struct,
        1,
        foundation,
    )?;
    for (index, id) in [
        (1, surface.foreign_callback_protocol.mode),
        (4, surface.foreign_callback_protocol.state),
    ] {
        validate_nominal_shape(
            CoreProtocolProductKindV1::ForeignCallback,
            CoreProtocolNominalV1::Type(id),
            index,
            SourceDeclarationKind::Enum,
            0,
            foundation,
        )?;
    }
    validate_nominal_shape(
        CoreProtocolProductKindV1::SourceLocation,
        CoreProtocolNominalV1::Type(surface.source_location_protocol.location),
        0,
        SourceDeclarationKind::Struct,
        0,
        foundation,
    )?;
    Ok(())
}

fn validate_nominal_shape(
    product: CoreProtocolProductKindV1,
    nominal: CoreProtocolNominalV1,
    index: usize,
    kind: SourceDeclarationKind,
    type_parameter_count: u32,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let source = match nominal {
        CoreProtocolNominalV1::Type(id) => foundation
            .source_type_by_bytes(id.as_array())
            .map(|(_, source)| source),
        CoreProtocolNominalV1::GenericType(id) => foundation
            .generic_type_by_bytes(id.as_array())
            .map(|(_, source)| source),
    }
    .expect("the nominal protocol entry was already resolved");
    if source.declaration_kind() == kind
        && source.duplicate_signature().type_parameter_count() == type_parameter_count
    {
        Ok(())
    } else {
        Err(CoreCompilerProtocolSurfaceValidationError::NominalRoleMismatch { product, index })
    }
}
