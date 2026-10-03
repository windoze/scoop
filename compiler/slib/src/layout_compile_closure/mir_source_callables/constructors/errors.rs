use super::*;
use scoop_wire::WireError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedMirConstructorComponent {
    Implementation,
    Owner,
    Execution,
    Receiver,
    ParameterCount,
    Parameter { index: usize },
    Result,
    GcEffect,
    LoweringRole,
    LoweredSignature,
    LoweredGcEffect,
}

#[derive(Debug)]
pub enum SharedMirConstructorValidationError {
    Resource(WireError),
    Shared(Box<hir::SharedTypeMetadataError>),
    Encoding(scoop_wire::cbor::EncodeError),
    Missing(PersistentConstructorId),
    Unexpected(PersistentConstructorId),
    Mismatch {
        declaration: PersistentConstructorId,
        component: SharedMirConstructorComponent,
    },
}

impl SharedMirConstructorValidationError {
    pub(super) fn require(
        declaration: PersistentConstructorId,
        component: SharedMirConstructorComponent,
        agrees: bool,
    ) -> Result<(), Self> {
        if agrees {
            Ok(())
        } else {
            Err(Self::Mismatch {
                declaration,
                component,
            })
        }
    }
}

impl From<WireError> for SharedMirConstructorValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<hir::SharedTypeMetadataError> for SharedMirConstructorValidationError {
    fn from(error: hir::SharedTypeMetadataError) -> Self {
        match error {
            hir::SharedTypeMetadataError::Resource(error) => Self::Resource(error),
            error => Self::Shared(Box::new(error)),
        }
    }
}
impl std::fmt::Display for SharedMirConstructorValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "shared HIR/MIR constructor agreement: {self:?}")
    }
}
impl std::error::Error for SharedMirConstructorValidationError {}
