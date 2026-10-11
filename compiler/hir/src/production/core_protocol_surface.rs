//! Complete typed declaration roles required by compiler protocols.

use scoop_identity::{
    CoreBuiltinNominal, PersistentDispatchSlotId, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentExactTypeId, PersistentGenericTypeId, PersistentTypeId,
};

use super::{CoreProtocolCallableBuildError, CoreProtocolCallableV1};
use crate::{
    ClassId, EnumId, EnumVariantFieldRef, EnumVariantRef, ExportHir, FunctionId,
    HirNominalIdentity, HirSourceNominalIdentity, InterfaceId, InterfaceMethodId,
    IntrinsicFunctionKind, MethodDispatch, StructId, TypeId,
};

mod fixed_signatures;
mod operation_owners;
mod operation_signatures;
mod shared_intrinsics;
mod source_types;
use fixed_signatures::validate_fixed_callable_signatures;
use operation_owners::expected_operation_owner;
pub use shared_intrinsics::IntrinsicCallableContractError;
mod validation;
mod wire;
use operation_signatures::expected_operation_signature;
use operation_signatures::operation_own_type_parameter_count;
pub use validation::{
    CoreCompilerProtocolSurfaceBuildError, CoreCompilerProtocolSurfaceRelationError,
    CoreProtocolProductKindV1,
};
pub use wire::{CoreCompilerProtocolSurfaceValidationError, DecodedCoreCompilerProtocolSurfaceV1};
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

pub(crate) const FUNDAMENTAL_TYPE_COUNT: usize = 20;
pub(crate) const OPTION_PROTOCOL_COUNT: usize = 4;
pub(crate) const ITERATION_PROTOCOL_COUNT: usize = 3;
pub(crate) const EXCEPTION_PROTOCOL_COUNT: usize = 17;
pub(crate) const COROUTINE_PROTOCOL_COUNT: usize = 13;
pub(crate) const FFI_PROTOCOL_COUNT: usize = 19;
pub(crate) const FOREIGN_CALLBACK_PROTOCOL_COUNT: usize = 15;
pub(crate) const SOURCE_LOCATION_PROTOCOL_COUNT: usize = 2;

/// Persistent nominal identity without an arena-id or FQN fallback.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CoreProtocolNominalV1 {
    Type(PersistentTypeId),
    GenericType(PersistentGenericTypeId),
}

/// Wire projection of a named role, also used to compare referenced subjects.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CoreProtocolEntryV1 {
    Nominal(CoreProtocolNominalV1),
    Callable(CoreProtocolCallableV1),
    EnumVariant(PersistentEnumVariantId),
    EnumVariantField(PersistentEnumVariantFieldId),
    DispatchSlot(PersistentDispatchSlotId),
    ExactType(PersistentExactTypeId),
}

mod roles;
pub use roles::*;
mod build;

impl CoreCoroutineProtocolV1 {
    pub fn continuation(&self) -> PersistentGenericTypeId {
        self.continuation
    }

    pub fn suspend_task(&self) -> PersistentGenericTypeId {
        self.suspend_task
    }
}

/// Complete typed declaration references for compiler protocols. Every
/// protocol family is a closed product, alongside the ordinary argv builder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoreCompilerProtocolSurfaceV1 {
    program_arguments: CoreProtocolCallableV1,
    fundamental_types: CoreFundamentalTypeProtocolV1,
    option_protocol: CoreOptionProtocolV1,
    iteration_protocol: CoreIterationProtocolV1,
    exception_protocol: CoreExceptionProtocolV1,
    coroutine_protocol: CoreCoroutineProtocolV1,
    ffi_protocol: CoreFfiProtocolV1,
    foreign_callback_protocol: CoreForeignCallbackProtocolV1,
    source_location_protocol: CoreSourceLocationProtocolV1,
}

impl CoreCompilerProtocolSurfaceV1 {
    pub const fn program_arguments(&self) -> &CoreProtocolCallableV1 {
        &self.program_arguments
    }

    pub const fn fundamental_types(&self) -> &CoreFundamentalTypeProtocolV1 {
        &self.fundamental_types
    }

    pub const fn option_protocol(&self) -> &CoreOptionProtocolV1 {
        &self.option_protocol
    }

    pub const fn iteration_protocol(&self) -> &CoreIterationProtocolV1 {
        &self.iteration_protocol
    }

    pub const fn exception_protocol(&self) -> &CoreExceptionProtocolV1 {
        &self.exception_protocol
    }

    pub const fn coroutine_protocol(&self) -> &CoreCoroutineProtocolV1 {
        &self.coroutine_protocol
    }

    pub const fn ffi_protocol(&self) -> &CoreFfiProtocolV1 {
        &self.ffi_protocol
    }

    pub const fn foreign_callback_protocol(&self) -> &CoreForeignCallbackProtocolV1 {
        &self.foreign_callback_protocol
    }

    pub const fn source_location_protocol(&self) -> &CoreSourceLocationProtocolV1 {
        &self.source_location_protocol
    }

    /// Compiler service used only by generated initialization
    /// cycle edges. It is deliberately independent of the public prelude.
    pub fn initialization_cycle_thrower(&self) -> &CoreProtocolCallableV1 {
        &self.exception_protocol.initialization_cycle_thrower
    }

    /// The actual declaration selected for a failed runtime cast.
    pub fn class_cast_exception_constructor(&self) -> &CoreProtocolCallableV1 {
        &self.exception_protocol.class_cast_exception_constructor
    }

    pub fn class_cast_exception_type(&self) -> PersistentTypeId {
        self.exception_protocol.class_cast_exception
    }

    pub fn string_source_type(&self) -> PersistentTypeId {
        self.fundamental_types.string
    }

    pub fn string_exact_type(&self) -> PersistentExactTypeId {
        PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(
            self.string_source_type(),
        ))
        .expect("a resolved nominal declaration has a canonical exact identity")
    }

    pub(crate) fn option_some(&self) -> PersistentEnumVariantId {
        self.option_protocol.some
    }

    pub(crate) fn option_some_payload(&self) -> PersistentEnumVariantFieldId {
        self.option_protocol.some_payload
    }

    pub(crate) fn option_none(&self) -> PersistentEnumVariantId {
        self.option_protocol.none
    }
}
