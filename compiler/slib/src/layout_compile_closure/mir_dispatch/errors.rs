use super::*;
use scoop_wire::WireError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedMirDispatchComponent {
    Tables,
    Slots,
    Position,
    SlotIdentity,
    Signature,
    Implementation,
    CallableSignature,
    CallableRole,
    CallableOrigin,
}

#[derive(Debug)]
pub enum SharedMirDispatchValidationError {
    Resource(WireError),
    Shared(Box<hir::SharedTypeMetadataError>),
    Lookup(mir::MirTypeBridgeLookupError),
    Key(scoop_identity::GeneratedCallableIdentityError),
    Missing(PersistentExactTypeId),
    Unexpected(PersistentExactTypeId),
    MissingFact(PersistentExactTypeId),
    MissingCallable(StrongCallableDefinitionOwner),
    UnexpectedAdjustment(PersistentGeneratedCallableId),
    AbstractSlot {
        owner: hir::SourceNominalId,
        slot: PersistentDispatchSlotId,
    },
    SourceSlot {
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    },
    Schema {
        owner: PersistentExactTypeId,
        component: SharedMirDispatchComponent,
    },
    Entry {
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
        component: SharedMirDispatchComponent,
    },
}

impl SharedMirDispatchValidationError {
    pub(super) fn schema(
        owner: PersistentExactTypeId,
        component: Component,
        agrees: bool,
    ) -> Result<(), Self> {
        if agrees {
            Ok(())
        } else {
            Err(Self::Schema { owner, component })
        }
    }
    pub(super) fn entry(
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
        component: Component,
        agrees: bool,
    ) -> Result<(), Self> {
        if agrees {
            Ok(())
        } else {
            Err(Self::Entry {
                owner,
                slot,
                component,
            })
        }
    }
}

impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<hir::SharedTypeMetadataError> for Error {
    fn from(error: hir::SharedTypeMetadataError) -> Self {
        match error {
            hir::SharedTypeMetadataError::Resource(error) => Self::Resource(error),
            error => Self::Shared(Box::new(error)),
        }
    }
}
impl From<mir::MirTypeBridgeLookupError> for Error {
    fn from(error: mir::MirTypeBridgeLookupError) -> Self {
        match error {
            mir::MirTypeBridgeLookupError::Resource(error) => Self::Resource(error),
            error => Self::Lookup(error),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "shared HIR/MIR dispatch agreement: {self:?}")
    }
}
impl std::error::Error for Error {}
