use std::fmt;

use super::*;

#[derive(Debug, Eq, PartialEq)]
pub enum CoreCompilerProtocolSurfaceValidationError {
    RoleKindMismatch {
        index: usize,
    },
    UnknownType([u8; 32]),
    UnknownGenericType([u8; 32]),
    UnknownEnumVariant([u8; 32]),
    UnknownEnumVariantField([u8; 32]),
    UnknownDispatchSlot([u8; 32]),
    UnknownExactType([u8; 32]),
    ExpectedSourceEnumOwner,
    GeneratedEnumMember,
    MissingDefinitionOrigin(DefinitionOriginSubject),
    NominalRoleMismatch {
        product: CoreProtocolProductKindV1,
        index: usize,
    },
    OptionOwnerMismatch,
    OptionPayloadMismatch,
    OptionVariantFieldCount {
        variant: PersistentEnumVariantId,
        expected: usize,
        actual: usize,
    },
    CallbackVariantOwnerMismatch {
        index: usize,
    },
    CallbackFailureTypeMismatch,
    InterfaceDispatchMismatch {
        product: CoreProtocolProductKindV1,
        callable_index: usize,
        slot_index: usize,
    },
    RoleCallableOwnerMismatch {
        product: CoreProtocolProductKindV1,
        index: usize,
    },
    OperationOwnerMismatch(IntrinsicFunctionKind),
    Callable(CoreProtocolCallableValidationError),
    Relation(CoreCompilerProtocolSurfaceRelationError),
}

impl fmt::Display for CoreCompilerProtocolSurfaceValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid core compiler protocol surface: {self:?}"
        )
    }
}

impl std::error::Error for CoreCompilerProtocolSurfaceValidationError {}
