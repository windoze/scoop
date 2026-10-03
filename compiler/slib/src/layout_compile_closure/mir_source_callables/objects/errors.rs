use super::*;
use scoop_identity::{PersistentGeneratedCallableId, PersistentTypeId};
use scoop_wire::WireError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedMirObjectComponent {
    Provider,
    Backing,
    ReadPlan,
    Unit,
    Ensure,
    CallableOrigin,
    CallableRole,
    Signature,
}

#[derive(Debug)]
pub enum SharedMirObjectValidationError {
    Resource(WireError),
    Shared(Box<hir::SharedTypeMetadataError>),
    Key(scoop_identity::GeneratedCallableIdentityError),
    MissingSource(PersistentTypeId),
    MissingUnit(PersistentTypeId),
    MissingObject(PersistentObjectValueId),
    MissingCallable(PersistentGeneratedCallableId),
    UnexpectedObject(PersistentObjectValueId),
    UnexpectedCallable(PersistentGeneratedCallableId),
    Object {
        value: PersistentObjectValueId,
        component: SharedMirObjectComponent,
    },
    Callable {
        callable: PersistentGeneratedCallableId,
        component: SharedMirObjectComponent,
    },
}

impl SharedMirObjectValidationError {
    pub(super) fn object(
        value: PersistentObjectValueId,
        component: SharedMirObjectComponent,
        agrees: bool,
    ) -> Result<(), Self> {
        if agrees {
            Ok(())
        } else {
            Err(Self::Object { value, component })
        }
    }
    pub(super) fn callable(
        callable: PersistentGeneratedCallableId,
        component: SharedMirObjectComponent,
        agrees: bool,
    ) -> Result<(), Self> {
        if agrees {
            Ok(())
        } else {
            Err(Self::Callable {
                callable,
                component,
            })
        }
    }
}
impl From<WireError> for SharedMirObjectValidationError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<hir::SharedTypeMetadataError> for SharedMirObjectValidationError {
    fn from(error: hir::SharedTypeMetadataError) -> Self {
        match error {
            hir::SharedTypeMetadataError::Resource(error) => Self::Resource(error),
            error => Self::Shared(Box::new(error)),
        }
    }
}
impl std::fmt::Display for SharedMirObjectValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "shared HIR/MIR object agreement: {self:?}")
    }
}
impl std::error::Error for SharedMirObjectValidationError {}
