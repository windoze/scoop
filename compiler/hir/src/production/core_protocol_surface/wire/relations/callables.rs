use super::*;

pub(super) fn validate_protocol_callable_owners(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let protocol = &surface.iteration_protocol;
    validate_callable_owner(
        CoreProtocolProductKindV1::Iteration,
        &protocol.next,
        1,
        DefinitionOwnerAtom::GenericType(protocol.iterator),
        foundation,
    )?;
    let protocol = &surface.coroutine_protocol;
    for (index, callable, owner) in [
        (1, &protocol.continuation_resume, protocol.continuation),
        (
            3,
            &protocol.continuation_resume_with_exception,
            protocol.continuation,
        ),
        (6, &protocol.suspend_task_run, protocol.suspend_task),
        (
            9,
            &protocol.suspend_registration_register,
            protocol.suspend_registration,
        ),
    ] {
        validate_callable_owner(
            CoreProtocolProductKindV1::Coroutine,
            callable,
            index,
            DefinitionOwnerAtom::GenericType(owner),
            foundation,
        )?;
    }
    let protocol = &surface.ffi_protocol;
    for (index, callable, owner) in [
        (4, &protocol.ptr_to_ulong, protocol.ptr),
        (5, &protocol.ptr_cast, protocol.ptr),
        (6, &protocol.ptr_load, protocol.ptr),
        (7, &protocol.ptr_load_offset, protocol.ptr),
        (8, &protocol.ptr_store, protocol.ptr),
        (9, &protocol.ptr_store_offset, protocol.ptr),
        (10, &protocol.ptr_plus, protocol.ptr),
        (11, &protocol.ptr_minus, protocol.ptr),
    ] {
        validate_callable_owner(
            CoreProtocolProductKindV1::Ffi,
            callable,
            index,
            DefinitionOwnerAtom::GenericType(owner),
            foundation,
        )?;
    }
    let protocol = &surface.exception_protocol;
    for (index, callable, owner) in [
        (1, &protocol.throwable_constructor, protocol.throwable),
        (
            3,
            &protocol.unwrap_exception_constructor,
            protocol.unwrap_exception,
        ),
        (
            5,
            &protocol.class_cast_exception_constructor,
            protocol.class_cast_exception,
        ),
        (
            7,
            &protocol.arithmetic_exception_constructor,
            protocol.arithmetic_exception,
        ),
        (
            9,
            &protocol.index_out_of_bounds_exception_constructor,
            protocol.index_out_of_bounds_exception,
        ),
        (
            11,
            &protocol.illegal_state_exception_constructor,
            protocol.illegal_state_exception,
        ),
        (
            14,
            &protocol.illegal_argument_exception_constructor,
            protocol.illegal_argument_exception,
        ),
    ] {
        validate_callable_owner(
            CoreProtocolProductKindV1::Exception,
            callable,
            index,
            DefinitionOwnerAtom::Type(owner),
            foundation,
        )?;
    }
    validate_top_level_callable(
        CoreProtocolProductKindV1::Exception,
        &surface.exception_protocol.initialization_cycle_thrower,
        12,
        foundation,
    )
}

pub(super) fn validate_interface_dispatch_relations(
    surface: &CoreCompilerProtocolSurfaceV1,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let protocol = &surface.iteration_protocol;
    validate_interface_dispatch(
        CoreProtocolProductKindV1::Iteration,
        (1, &protocol.next),
        (2, protocol.next_dispatch),
        foundation,
    )?;
    let protocol = &surface.coroutine_protocol;
    for (callable, slot) in [
        (
            (1, &protocol.continuation_resume),
            (2, protocol.continuation_resume_dispatch),
        ),
        (
            (3, &protocol.continuation_resume_with_exception),
            (4, protocol.continuation_resume_with_exception_dispatch),
        ),
        (
            (6, &protocol.suspend_task_run),
            (7, protocol.suspend_task_run_dispatch),
        ),
        (
            (9, &protocol.suspend_registration_register),
            (10, protocol.suspend_registration_register_dispatch),
        ),
    ] {
        validate_interface_dispatch(
            CoreProtocolProductKindV1::Coroutine,
            callable,
            slot,
            foundation,
        )?;
    }
    Ok(())
}

fn validate_top_level_callable(
    product: CoreProtocolProductKindV1,
    callable: &CoreProtocolCallableV1,
    callable_index: usize,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let source = protocol_callable_source(foundation, callable.definition()).ok_or(
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

fn validate_callable_owner(
    product: CoreProtocolProductKindV1,
    callable: &CoreProtocolCallableV1,
    callable_index: usize,
    expected: DefinitionOwnerAtom,
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let source = protocol_callable_source(foundation, callable.definition()).ok_or(
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

pub(super) fn protocol_callable_source(
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

fn validate_interface_dispatch(
    product: CoreProtocolProductKindV1,
    (callable_index, callable): (usize, &CoreProtocolCallableV1),
    (slot_index, slot): (usize, PersistentDispatchSlotId),
    foundation: &CanonicalHirFoundation,
) -> Result<(), CoreCompilerProtocolSurfaceValidationError> {
    let definition = callable.definition();
    let CoreProtocolCallableDefinitionV1::Function(function) = definition else {
        return Err(CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::RoleCallableKindMismatch {
                product,
                index: callable_index,
            },
        ));
    };
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
