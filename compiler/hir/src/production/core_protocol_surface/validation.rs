use std::fmt;

use super::*;

impl CoreCompilerProtocolSurfaceV1 {
    pub(super) fn validate_internal_relations(
        &self,
    ) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
        // Repetition across protocols is intentional only for callables that
        // also appear in the total intrinsic operation table; fixed products
        // themselves must not repeat one subject under two roles.
        for entries in [
            self.fundamental_types.entries().as_slice(),
            self.option_protocol.entries().as_slice(),
            self.iteration_protocol.entries().as_slice(),
            self.exception_protocol.entries().as_slice(),
            self.coroutine_protocol.entries().as_slice(),
            self.ffi_protocol.entries().as_slice(),
            self.foreign_callback_protocol.entries().as_slice(),
            self.source_location_protocol.entries().as_slice(),
        ] {
            let mut entries = entries.to_vec();
            entries.sort_unstable();
            if entries.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(CoreCompilerProtocolSurfaceRelationError::DuplicateRoleSubject);
            }
        }
        require_source_callables(
            CoreProtocolProductKindV1::Iteration,
            self.iteration_protocol.entries(),
            &[1],
        )?;
        require_constructor_callables(
            CoreProtocolProductKindV1::Exception,
            self.exception_protocol.entries(),
            &[1, 3, 5, 7, 9, 11],
        )?;
        require_source_callables(
            CoreProtocolProductKindV1::Exception,
            self.exception_protocol.entries(),
            &[12],
        )?;
        require_source_callables(
            CoreProtocolProductKindV1::Coroutine,
            self.coroutine_protocol.entries(),
            &[1, 3, 6, 9, 11, 12],
        )?;
        require_source_callables(
            CoreProtocolProductKindV1::Ffi,
            self.ffi_protocol.entries(),
            &(4..FFI_PROTOCOL_COUNT).collect::<Vec<_>>(),
        )?;
        require_source_callables(
            CoreProtocolProductKindV1::ForeignCallback,
            self.foreign_callback_protocol.entries(),
            &(10..FOREIGN_CALLBACK_PROTOCOL_COUNT).collect::<Vec<_>>(),
        )?;
        require_source_callables(
            CoreProtocolProductKindV1::SourceLocation,
            self.source_location_protocol.entries(),
            &[1],
        )?;
        validate_operation_set(&self.compiler_operation_protocol.operations)?;
        self.validate_repeated_operation_relations()?;
        Ok(())
    }

    pub(super) fn validate_operation_signatures(
        &self,
    ) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
        for operation in &self.compiler_operation_protocol.operations {
            let expected = expected_operation_signature(self, operation.kind);
            if operation.callable.signature() != &expected {
                return Err(
                    CoreCompilerProtocolSurfaceRelationError::OperationSignatureMismatch(
                        operation.kind,
                    ),
                );
            }
        }
        Ok(())
    }

    fn validate_repeated_operation_relations(
        &self,
    ) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
        let coroutine = self.coroutine_protocol.entries();
        let ffi = self.ffi_protocol.entries();
        let callback = self.foreign_callback_protocol.entries();
        let location = self.source_location_protocol.entries();
        for (kind, callable) in [
            (
                IntrinsicFunctionKind::CoroutineStart,
                callable_entry_ref(coroutine, 11),
            ),
            (
                IntrinsicFunctionKind::CoroutineSuspend,
                callable_entry_ref(coroutine, 12),
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::ToULong),
                callable_entry_ref(ffi, 4),
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Cast),
                callable_entry_ref(ffi, 5),
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Load),
                callable_entry_ref(ffi, 6),
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::LoadOffset),
                callable_entry_ref(ffi, 7),
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Store),
                callable_entry_ref(ffi, 8),
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::StoreOffset),
                callable_entry_ref(ffi, 9),
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Plus),
                callable_entry_ref(ffi, 10),
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Minus),
                callable_entry_ref(ffi, 11),
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::AddressOf),
                callable_entry_ref(ffi, 12),
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::SizeOf),
                callable_entry_ref(ffi, 13),
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::AlignOf),
                callable_entry_ref(ffi, 14),
            ),
            (IntrinsicFunctionKind::GcPinRaw, callable_entry_ref(ffi, 15)),
            (
                IntrinsicFunctionKind::GcUnpinRaw,
                callable_entry_ref(ffi, 16),
            ),
            (
                IntrinsicFunctionKind::GcGetHandleRaw,
                callable_entry_ref(ffi, 17),
            ),
            (
                IntrinsicFunctionKind::GcReleaseHandleRaw,
                callable_entry_ref(ffi, 18),
            ),
            (
                IntrinsicFunctionKind::ForeignCallbackRegister,
                callable_entry_ref(callback, 10),
            ),
            (
                IntrinsicFunctionKind::ForeignCallbackRetain,
                callable_entry_ref(callback, 11),
            ),
            (
                IntrinsicFunctionKind::ForeignCallbackRelease,
                callable_entry_ref(callback, 12),
            ),
            (
                IntrinsicFunctionKind::ForeignCallbackState,
                callable_entry_ref(callback, 13),
            ),
            (
                IntrinsicFunctionKind::ForeignCallbackFailure,
                callable_entry_ref(callback, 14),
            ),
            (
                IntrinsicFunctionKind::CurrentSourceLocation,
                callable_entry_ref(location, 1),
            ),
        ] {
            let Some(operation) = self
                .compiler_operation_protocol
                .operations
                .iter()
                .find(|operation| operation.kind == kind)
            else {
                return Err(
                    CoreCompilerProtocolSurfaceRelationError::OperationCoverage {
                        expected: intrinsic_function_kinds().len(),
                        actual: self.compiler_operation_protocol.operations.len(),
                    },
                );
            };
            if operation.callable != *callable {
                return Err(
                    CoreCompilerProtocolSurfaceRelationError::RepeatedOperationMismatch(kind),
                );
            }
        }
        Ok(())
    }
}

pub(super) fn validate_operation_set(
    operations: &[CoreCompilerOperationV1],
) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
    let expected = intrinsic_function_kinds();
    if operations.len() != expected.len() {
        return Err(
            CoreCompilerProtocolSurfaceRelationError::OperationCoverage {
                expected: expected.len(),
                actual: operations.len(),
            },
        );
    }
    let mut definitions = Vec::with_capacity(operations.len());
    for (index, (operation, expected)) in operations.iter().zip(expected).enumerate() {
        if operation.kind != expected {
            return Err(
                CoreCompilerProtocolSurfaceRelationError::OperationRoleMismatch {
                    index,
                    expected,
                    actual: operation.kind,
                },
            );
        }
        let expected_effect = if expected == IntrinsicFunctionKind::CoroutineSuspend {
            scoop_identity::Effect::Suspend
        } else {
            scoop_identity::Effect::Ordinary
        };
        if operation.callable.signature().effect() != expected_effect {
            return Err(
                CoreCompilerProtocolSurfaceRelationError::OperationEffectMismatch(expected),
            );
        }
        let receiver_expected = intrinsic_receiver_is_present(expected);
        if operation.callable.signature().receiver().is_present() != receiver_expected {
            return Err(
                CoreCompilerProtocolSurfaceRelationError::OperationReceiverMismatch(expected),
            );
        }
        if !matches!(
            operation.callable.definition(),
            super::super::CoreProtocolCallableDefinitionV1::Function(_)
                | super::super::CoreProtocolCallableDefinitionV1::GenericFunction(_)
        ) {
            return Err(
                CoreCompilerProtocolSurfaceRelationError::OperationCallableKindMismatch(expected),
            );
        }
        definitions.push(operation.callable.definition());
    }
    definitions.sort_unstable();
    if let Some(pair) = definitions.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(CoreCompilerProtocolSurfaceRelationError::DuplicateOperationCallable(pair[0]));
    }
    Ok(())
}

fn intrinsic_receiver_is_present(_kind: IntrinsicFunctionKind) -> bool {
    // SignatureCallableShape.receiver is the explicit extension receiver.
    // Member ownership is carried by the source declaration owner chain, so
    // every current intrinsic role has an absent signature receiver.
    false
}

fn require_source_callables<const N: usize>(
    product: CoreProtocolProductKindV1,
    entries: &[CoreProtocolEntryV1; N],
    indices: &[usize],
) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
    for &index in indices {
        if !matches!(
            callable_entry_ref(entries, index).definition(),
            super::super::CoreProtocolCallableDefinitionV1::Function(_)
                | super::super::CoreProtocolCallableDefinitionV1::GenericFunction(_)
        ) {
            return Err(
                CoreCompilerProtocolSurfaceRelationError::RoleCallableKindMismatch {
                    product,
                    index,
                },
            );
        }
    }
    Ok(())
}

fn require_constructor_callables<const N: usize>(
    product: CoreProtocolProductKindV1,
    entries: &[CoreProtocolEntryV1; N],
    indices: &[usize],
) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
    for &index in indices {
        if !matches!(
            callable_entry_ref(entries, index).definition(),
            super::super::CoreProtocolCallableDefinitionV1::Constructor(_)
                | super::super::CoreProtocolCallableDefinitionV1::GeneratedCallable(_)
        ) {
            return Err(
                CoreCompilerProtocolSurfaceRelationError::RoleCallableKindMismatch {
                    product,
                    index,
                },
            );
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum CoreCompilerProtocolSurfaceBuildError {
    NotCore(scoop_identity::ConeIdentity),
    Callable(CoreProtocolCallableBuildError),
    GeneratedProtocolNominal,
    ProtocolNominalKindMismatch,
    MissingInterfaceDispatch { function: u32 },
    OpenProtocolType { ty: u32 },
    Relation(CoreCompilerProtocolSurfaceRelationError),
}

impl fmt::Display for CoreCompilerProtocolSurfaceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot build core compiler protocol surface: {self:?}"
        )
    }
}

impl std::error::Error for CoreCompilerProtocolSurfaceBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreProtocolProductKindV1 {
    Fundamental,
    Option,
    Iteration,
    Exception,
    Coroutine,
    Ffi,
    ForeignCallback,
    SourceLocation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreCompilerProtocolSurfaceRelationError {
    DuplicateRoleSubject,
    RoleCallableKindMismatch {
        product: CoreProtocolProductKindV1,
        index: usize,
    },
    OperationCoverage {
        expected: usize,
        actual: usize,
    },
    OperationRoleMismatch {
        index: usize,
        expected: IntrinsicFunctionKind,
        actual: IntrinsicFunctionKind,
    },
    OperationEffectMismatch(IntrinsicFunctionKind),
    OperationReceiverMismatch(IntrinsicFunctionKind),
    OperationSignatureMismatch(IntrinsicFunctionKind),
    OperationCallableKindMismatch(IntrinsicFunctionKind),
    DuplicateOperationCallable(super::super::CoreProtocolCallableDefinitionV1),
    RepeatedOperationMismatch(IntrinsicFunctionKind),
    FixedCallableSignatureMismatch {
        product: CoreProtocolProductKindV1,
        index: usize,
    },
}

impl fmt::Display for CoreCompilerProtocolSurfaceRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid core compiler protocol relation: {self:?}"
        )
    }
}

impl std::error::Error for CoreCompilerProtocolSurfaceRelationError {}
