use std::fmt;

use super::*;

impl CoreCompilerProtocolSurfaceV1 {
    pub(super) fn validate_internal_relations(
        &self,
    ) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
        // Fixed products must not repeat one subject under two roles.
        for entries in [
            self.fundamental_types.wire_entries().as_slice(),
            self.option_protocol.wire_entries().as_slice(),
            self.iteration_protocol.wire_entries().as_slice(),
            self.exception_protocol.wire_entries().as_slice(),
            self.coroutine_protocol.wire_entries().as_slice(),
            self.ffi_protocol.wire_entries().as_slice(),
            self.foreign_callback_protocol.wire_entries().as_slice(),
            self.source_location_protocol.wire_entries().as_slice(),
        ] {
            let mut entries = entries.to_vec();
            entries.sort_unstable();
            if entries.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(CoreCompilerProtocolSurfaceRelationError::DuplicateRoleSubject);
            }
        }
        require_source_callables(
            CoreProtocolProductKindV1::Iteration,
            &[(1, &self.iteration_protocol.next)],
        )?;
        require_constructor_callables(
            CoreProtocolProductKindV1::Exception,
            &[
                (1, &self.exception_protocol.throwable_constructor),
                (3, &self.exception_protocol.unwrap_exception_constructor),
                (5, &self.exception_protocol.class_cast_exception_constructor),
                (7, &self.exception_protocol.arithmetic_exception_constructor),
                (
                    9,
                    &self
                        .exception_protocol
                        .index_out_of_bounds_exception_constructor,
                ),
                (
                    11,
                    &self.exception_protocol.illegal_state_exception_constructor,
                ),
            ],
        )?;
        require_source_callables(
            CoreProtocolProductKindV1::Exception,
            &[(12, &self.exception_protocol.initialization_cycle_thrower)],
        )?;
        require_source_callables(
            CoreProtocolProductKindV1::Coroutine,
            &[
                (1, &self.coroutine_protocol.continuation_resume),
                (
                    3,
                    &self.coroutine_protocol.continuation_resume_with_exception,
                ),
                (6, &self.coroutine_protocol.suspend_task_run),
                (9, &self.coroutine_protocol.suspend_registration_register),
                (11, &self.coroutine_protocol.start_coroutine),
                (12, &self.coroutine_protocol.suspend_coroutine),
            ],
        )?;
        require_source_callables(
            CoreProtocolProductKindV1::Ffi,
            &[
                (4, &self.ffi_protocol.ptr_to_ulong),
                (5, &self.ffi_protocol.ptr_cast),
                (6, &self.ffi_protocol.ptr_load),
                (7, &self.ffi_protocol.ptr_load_offset),
                (8, &self.ffi_protocol.ptr_store),
                (9, &self.ffi_protocol.ptr_store_offset),
                (10, &self.ffi_protocol.ptr_plus),
                (11, &self.ffi_protocol.ptr_minus),
                (12, &self.ffi_protocol.address_of),
                (13, &self.ffi_protocol.size_of),
                (14, &self.ffi_protocol.align_of),
                (15, &self.ffi_protocol.gc_pin_raw),
                (16, &self.ffi_protocol.gc_unpin_raw),
                (17, &self.ffi_protocol.gc_get_handle_raw),
                (18, &self.ffi_protocol.gc_release_handle_raw),
            ],
        )?;
        require_source_callables(
            CoreProtocolProductKindV1::ForeignCallback,
            &[
                (10, &self.foreign_callback_protocol.register),
                (11, &self.foreign_callback_protocol.retain),
                (12, &self.foreign_callback_protocol.release),
                (13, &self.foreign_callback_protocol.query_state),
                (14, &self.foreign_callback_protocol.failure),
            ],
        )?;
        require_source_callables(
            CoreProtocolProductKindV1::SourceLocation,
            &[(1, &self.source_location_protocol.current)],
        )?;
        Ok(())
    }

    pub(super) fn validate_fixed_intrinsic_signatures(
        &self,
    ) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
        for (kind, callable) in self.fixed_intrinsic_callables() {
            if callable.signature() != &expected_operation_signature(self, kind) {
                return Err(
                    CoreCompilerProtocolSurfaceRelationError::OperationSignatureMismatch(kind),
                );
            }
        }
        Ok(())
    }

    pub(super) fn fixed_intrinsic_callables(
        &self,
    ) -> [(IntrinsicFunctionKind, &CoreProtocolCallableV1); 23] {
        let coroutine = &self.coroutine_protocol;
        let ffi = &self.ffi_protocol;
        let callback = &self.foreign_callback_protocol;
        let location = &self.source_location_protocol;
        [
            (
                IntrinsicFunctionKind::CoroutineStart,
                &coroutine.start_coroutine,
            ),
            (
                IntrinsicFunctionKind::CoroutineSuspend,
                &coroutine.suspend_coroutine,
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::ToULong),
                &ffi.ptr_to_ulong,
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Cast),
                &ffi.ptr_cast,
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Load),
                &ffi.ptr_load,
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::LoadOffset),
                &ffi.ptr_load_offset,
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Store),
                &ffi.ptr_store,
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::StoreOffset),
                &ffi.ptr_store_offset,
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Plus),
                &ffi.ptr_plus,
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::Minus),
                &ffi.ptr_minus,
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::AddressOf),
                &ffi.address_of,
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::SizeOf),
                &ffi.size_of,
            ),
            (
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::AlignOf),
                &ffi.align_of,
            ),
            (IntrinsicFunctionKind::GcPinRaw, &ffi.gc_pin_raw),
            (IntrinsicFunctionKind::GcUnpinRaw, &ffi.gc_unpin_raw),
            (
                IntrinsicFunctionKind::GcGetHandleRaw,
                &ffi.gc_get_handle_raw,
            ),
            (
                IntrinsicFunctionKind::GcReleaseHandleRaw,
                &ffi.gc_release_handle_raw,
            ),
            (
                IntrinsicFunctionKind::ForeignCallbackRegister,
                &callback.register,
            ),
            (
                IntrinsicFunctionKind::ForeignCallbackRetain,
                &callback.retain,
            ),
            (
                IntrinsicFunctionKind::ForeignCallbackRelease,
                &callback.release,
            ),
            (
                IntrinsicFunctionKind::ForeignCallbackState,
                &callback.query_state,
            ),
            (
                IntrinsicFunctionKind::ForeignCallbackFailure,
                &callback.failure,
            ),
            (
                IntrinsicFunctionKind::CurrentSourceLocation,
                &location.current,
            ),
        ]
    }
}

fn require_source_callables(
    product: CoreProtocolProductKindV1,
    callables: &[(usize, &CoreProtocolCallableV1)],
) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
    for &(index, callable) in callables {
        if !matches!(
            callable.definition(),
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

fn require_constructor_callables(
    product: CoreProtocolProductKindV1,
    callables: &[(usize, &CoreProtocolCallableV1)],
) -> Result<(), CoreCompilerProtocolSurfaceRelationError> {
    for &(index, callable) in callables {
        if !matches!(
            callable.definition(),
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
    ProgramArgumentsSignature,
    TypeReferenceSignature,
    DuplicateRoleSubject,
    RoleCallableKindMismatch {
        product: CoreProtocolProductKindV1,
        index: usize,
    },
    OperationSignatureMismatch(IntrinsicFunctionKind),
    OperationCallableKindMismatch(IntrinsicFunctionKind),
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
