use super::*;

pub(super) fn validate_foundation_relations(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    validate_nominal_relations(surface, foundation)?;
    validate_option_relations(surface, foundation)?;
    validate_foreign_callback_relations(surface, foundation)?;
    validate_protocol_callable_owners(surface, foundation)?;
    validate_interface_dispatch_relations(surface, foundation)?;
    for (kind, callable) in surface.fixed_intrinsic_callables() {
        validate_operation_owner(surface, foundation, kind, callable)?;
    }
    Ok(())
}

fn validate_foreign_callback_relations(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let entries = surface.foreign_callback_protocol.entries();
    validate_variant_owners(entries, 1, &[2, 3], foundation)?;
    validate_variant_owners(entries, 4, &[5, 6, 7, 8], foundation)?;

    let failure = match entries[9] {
        CoreProtocolEntryV1::ExactType(id) => id,
        _ => unreachable!("the callback failure role was kind-validated as an exact type"),
    };
    let (_, failure_key) = foundation
        .exact_type_by_bytes(failure.as_array())
        .expect("the callback failure exact type was resolved");
    let option = generic_nominal_id(surface.option_protocol.entries(), 0);
    let throwable = concrete_nominal_id(surface.exception_protocol.entries(), 0);
    let valid = match failure_key {
        scoop_identity::ExactTypeKey::NominalApplication { origin, arguments }
            if *origin == option && arguments.as_slice().len() == 1 =>
        {
            foundation
                .exact_type_by_bytes(arguments.as_slice()[0].as_array())
                .is_some_and(|(_, key)| key == &scoop_identity::ExactTypeKey::Nominal(throwable))
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(CoreCompilerProtocolSurfaceValidationError::CallbackFailureTypeMismatch)
    }
}

fn validate_variant_owners<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    owner_index: usize,
    variant_indices: &[usize],
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let owner = NominalDeclarationOwner::Concrete(concrete_nominal_id(entries, owner_index));
    for &index in variant_indices {
        let variant = variant_id(entries, index);
        let (_, key) = foundation
            .enum_variant_by_bytes(variant.as_array())
            .expect("the callback enum variant was resolved");
        if key.source_owner() != Some(owner) {
            return Err(
                CoreCompilerProtocolSurfaceValidationError::CallbackVariantOwnerMismatch { index },
            );
        }
    }
    Ok(())
}

fn validate_nominal_relations(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let fundamental = surface.fundamental_types.entries();
    if concrete_nominal_id(fundamental, 0) != CoreBuiltinNominal::Unit.identity_record().id() {
        return Err(
            CoreCompilerProtocolSurfaceValidationError::NominalRoleMismatch {
                product: CoreProtocolProductKindV1::Fundamental,
                index: 0,
            },
        );
    }
    for index in 0..=9 {
        validate_nominal_shape(
            CoreProtocolProductKindV1::Fundamental,
            fundamental,
            index,
            SourceDeclarationKind::Struct,
            0,
            foundation,
        )?;
    }
    validate_nominal_shape(
        CoreProtocolProductKindV1::Fundamental,
        fundamental,
        10,
        SourceDeclarationKind::Class,
        0,
        foundation,
    )?;
    for index in [11, 12] {
        validate_nominal_shape(
            CoreProtocolProductKindV1::Fundamental,
            fundamental,
            index,
            SourceDeclarationKind::Class,
            1,
            foundation,
        )?;
    }
    for index in [13, 14] {
        validate_nominal_shape(
            CoreProtocolProductKindV1::Fundamental,
            fundamental,
            index,
            SourceDeclarationKind::Struct,
            1,
            foundation,
        )?;
    }
    validate_nominal_shape(
        CoreProtocolProductKindV1::Option,
        surface.option_protocol.entries(),
        0,
        SourceDeclarationKind::Enum,
        1,
        foundation,
    )?;
    validate_nominal_shape(
        CoreProtocolProductKindV1::Iteration,
        surface.iteration_protocol.entries(),
        0,
        SourceDeclarationKind::Interface,
        1,
        foundation,
    )?;
    for index in [0, 2, 4, 6, 8, 10, 13] {
        validate_nominal_shape(
            CoreProtocolProductKindV1::Exception,
            surface.exception_protocol.entries(),
            index,
            SourceDeclarationKind::Class,
            0,
            foundation,
        )?;
    }
    for index in [0, 5, 8] {
        validate_nominal_shape(
            CoreProtocolProductKindV1::Coroutine,
            surface.coroutine_protocol.entries(),
            index,
            SourceDeclarationKind::Interface,
            1,
            foundation,
        )?;
    }
    for index in 0..4 {
        validate_nominal_shape(
            CoreProtocolProductKindV1::Ffi,
            surface.ffi_protocol.entries(),
            index,
            SourceDeclarationKind::Struct,
            1,
            foundation,
        )?;
    }
    validate_nominal_shape(
        CoreProtocolProductKindV1::ForeignCallback,
        surface.foreign_callback_protocol.entries(),
        0,
        SourceDeclarationKind::Struct,
        1,
        foundation,
    )?;
    for index in [1, 4] {
        validate_nominal_shape(
            CoreProtocolProductKindV1::ForeignCallback,
            surface.foreign_callback_protocol.entries(),
            index,
            SourceDeclarationKind::Enum,
            0,
            foundation,
        )?;
    }
    validate_nominal_shape(
        CoreProtocolProductKindV1::SourceLocation,
        surface.source_location_protocol.entries(),
        0,
        SourceDeclarationKind::Struct,
        0,
        foundation,
    )
}

fn validate_nominal_shape<const N: usize>(
    product: CoreProtocolProductKindV1,
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
    kind: SourceDeclarationKind,
    type_parameter_count: u32,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let source = match entries[index] {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(id)) => foundation
            .source_type_by_bytes(id.as_array())
            .map(|(_, source)| source),
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => foundation
            .generic_type_by_bytes(id.as_array())
            .map(|(_, source)| source),
        _ => None,
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

fn validate_protocol_callable_owners(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    validate_callable_owner(
        CoreProtocolProductKindV1::Iteration,
        surface.iteration_protocol.entries(),
        1,
        DefinitionOwnerAtom::GenericType(generic_nominal_id(
            surface.iteration_protocol.entries(),
            0,
        )),
        foundation,
    )?;
    for (callable, owner) in [(1, 0), (3, 0), (6, 5), (9, 8)] {
        validate_callable_owner(
            CoreProtocolProductKindV1::Coroutine,
            surface.coroutine_protocol.entries(),
            callable,
            DefinitionOwnerAtom::GenericType(generic_nominal_id(
                surface.coroutine_protocol.entries(),
                owner,
            )),
            foundation,
        )?;
    }
    for callable in 4..12 {
        validate_callable_owner(
            CoreProtocolProductKindV1::Ffi,
            surface.ffi_protocol.entries(),
            callable,
            DefinitionOwnerAtom::GenericType(generic_nominal_id(surface.ffi_protocol.entries(), 0)),
            foundation,
        )?;
    }
    for (callable, owner) in [(1, 0), (3, 2), (5, 4), (7, 6), (9, 8), (11, 10), (14, 13)] {
        validate_callable_owner(
            CoreProtocolProductKindV1::Exception,
            surface.exception_protocol.entries(),
            callable,
            DefinitionOwnerAtom::Type(concrete_nominal_id(
                surface.exception_protocol.entries(),
                owner,
            )),
            foundation,
        )?;
    }
    validate_top_level_callable(
        CoreProtocolProductKindV1::Exception,
        surface.exception_protocol.entries(),
        12,
        foundation,
    )?;
    Ok(())
}

fn validate_top_level_callable<const N: usize>(
    product: CoreProtocolProductKindV1,
    entries: &[CoreProtocolEntryV1; N],
    callable_index: usize,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let source = protocol_callable_source(
        foundation,
        callable_entry_ref(entries, callable_index).definition(),
    )
    .ok_or(
        CoreCompilerProtocolSurfaceValidationError::RoleCallableOwnerMismatch {
            product,
            index: callable_index,
        },
    )?;
    if source.owners().owners().is_empty() {
        Ok(())
    } else {
        Err(
            CoreCompilerProtocolSurfaceValidationError::RoleCallableOwnerMismatch {
                product,
                index: callable_index,
            },
        )
    }
}

fn validate_callable_owner<const N: usize>(
    product: CoreProtocolProductKindV1,
    entries: &[CoreProtocolEntryV1; N],
    callable_index: usize,
    expected: DefinitionOwnerAtom,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let source = protocol_callable_source(
        foundation,
        callable_entry_ref(entries, callable_index).definition(),
    )
    .ok_or(
        CoreCompilerProtocolSurfaceValidationError::RoleCallableOwnerMismatch {
            product,
            index: callable_index,
        },
    )?;
    if source.owners().owners() == [expected] {
        Ok(())
    } else {
        Err(
            CoreCompilerProtocolSurfaceValidationError::RoleCallableOwnerMismatch {
                product,
                index: callable_index,
            },
        )
    }
}

fn protocol_callable_source(
    foundation: &CanonicalHirFoundation,
    definition: CoreProtocolCallableDefinitionV1,
) -> Option<&scoop_identity::SourceDeclarationKey> {
    match definition {
        CoreProtocolCallableDefinitionV1::Function(id) => foundation
            .function_by_bytes(id.as_array())
            .map(|(_, source)| source),
        CoreProtocolCallableDefinitionV1::GenericFunction(id) => foundation
            .generic_function_by_bytes(id.as_array())
            .map(|(_, source)| source),
        CoreProtocolCallableDefinitionV1::Constructor(id) => foundation
            .constructor_by_bytes(id.as_array())
            .map(|(_, source)| source),
        CoreProtocolCallableDefinitionV1::GeneratedCallable(id) => {
            let (_, key) = foundation.generated_callable_by_bytes(id.as_array())?;
            let scoop_identity::GeneratedCallableKey::ZeroArgumentConstructorAdapter {
                constructor,
            } = key
            else {
                return None;
            };
            foundation
                .constructor_by_bytes(constructor.as_array())
                .map(|(_, source)| source)
        }
    }
}

fn validate_option_relations(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let entries = surface.option_protocol.entries();
    let option = match entries[0] {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => {
            NominalDeclarationOwner::GenericTemplate(id)
        }
        _ => unreachable!("the Option owner role was kind-validated"),
    };
    let some = surface.option_some();
    let none = surface.option_none();
    for variant in [some, none] {
        let (_, key) = foundation
            .enum_variant_by_bytes(variant.as_array())
            .expect("the Option variant was resolved");
        if key.source_owner() != Some(option) {
            return Err(CoreCompilerProtocolSurfaceValidationError::OptionOwnerMismatch);
        }
    }
    for (variant, expected) in [(some, 1), (none, 0)] {
        let actual = foundation.enum_variant_field_count(variant);
        if actual != expected {
            return Err(
                CoreCompilerProtocolSurfaceValidationError::OptionVariantFieldCount {
                    variant,
                    expected,
                    actual,
                },
            );
        }
    }
    let field = surface.option_some_payload();
    let (_, key) = foundation
        .enum_variant_field_by_bytes(field.as_array())
        .expect("the Option payload field was resolved");
    if key.variant() != some
        || key.selector()
            != &(EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            })
    {
        return Err(CoreCompilerProtocolSurfaceValidationError::OptionPayloadMismatch);
    }
    Ok(())
}

fn validate_interface_dispatch_relations(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    validate_interface_dispatch(
        CoreProtocolProductKindV1::Iteration,
        surface.iteration_protocol.entries(),
        1,
        2,
        foundation,
    )?;
    for (callable, slot) in [(1, 2), (3, 4), (6, 7), (9, 10)] {
        validate_interface_dispatch(
            CoreProtocolProductKindV1::Coroutine,
            surface.coroutine_protocol.entries(),
            callable,
            slot,
            foundation,
        )?;
    }
    Ok(())
}

fn validate_interface_dispatch<const N: usize>(
    product: CoreProtocolProductKindV1,
    entries: &[CoreProtocolEntryV1; N],
    callable_index: usize,
    slot_index: usize,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let definition = callable_entry_ref(entries, callable_index).definition();
    let CoreProtocolCallableDefinitionV1::Function(function) = definition else {
        return Err(CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::RoleCallableKindMismatch {
                product,
                index: callable_index,
            },
        ));
    };
    let slot = dispatch_slot_id(entries, slot_index);
    let (_, key) = foundation
        .dispatch_slot_by_bytes(slot.as_array())
        .expect("the dispatch slot was resolved");
    if key.owner() != DispatchDeclarationOwner::Function(function)
        || key.role() != DispatchRole::InterfaceMethod
    {
        return Err(
            CoreCompilerProtocolSurfaceValidationError::InterfaceDispatchMismatch {
                product,
                callable_index,
                slot_index,
            },
        );
    }
    Ok(())
}

fn validate_operation_owner(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
    kind: IntrinsicFunctionKind,
    callable: &CoreProtocolCallableV1,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let source = protocol_callable_source(foundation, callable.definition()).ok_or(
        CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::OperationCallableKindMismatch(kind),
        ),
    )?;
    let expected = expected_operation_owner(surface, kind);
    let owners = source.owners().owners();
    let matches = match expected {
        None => owners.is_empty(),
        Some(expected) => owners == [expected],
    };
    if matches {
        Ok(())
    } else {
        Err(CoreCompilerProtocolSurfaceValidationError::OperationOwnerMismatch(kind))
    }
}

fn concrete_nominal_id<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentTypeId {
    match entries[index] {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::Type(id)) => id,
        _ => unreachable!("the fixed role was kind-validated as a concrete nominal"),
    }
}

fn generic_nominal_id<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentGenericTypeId {
    match entries[index] {
        CoreProtocolEntryV1::Nominal(CoreProtocolNominalV1::GenericType(id)) => id,
        _ => unreachable!("the fixed role was kind-validated as a generic nominal"),
    }
}

fn variant_id<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentEnumVariantId {
    match entries[index] {
        CoreProtocolEntryV1::EnumVariant(id) => id,
        _ => unreachable!("the fixed role was kind-validated as an enum variant"),
    }
}

fn dispatch_slot_id<const N: usize>(
    entries: &[CoreProtocolEntryV1; N],
    index: usize,
) -> PersistentDispatchSlotId {
    match entries[index] {
        CoreProtocolEntryV1::DispatchSlot(id) => id,
        _ => unreachable!("the fixed role was kind-validated as a dispatch slot"),
    }
}
