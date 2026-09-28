use super::*;

#[derive(Debug)]
pub enum MirDispatchSchemaError {
    DefinitionReference(scoop_identity::CallableBodyResolutionError<IdentityReferenceError>),
    Reference(IdentityReferenceError),
    Resource(WireError),
    Lookup(MirTypeBridgeLookupError),
    Signature(Box<MirCallableBridgeError>),
    MissingType {
        exact: PersistentExactTypeId,
    },
    MissingCallable {
        target: CallableDefinitionOwner,
    },
    MissingSchema {
        owner: PersistentExactTypeId,
    },
    MissingInterfaceTable {
        owner: PersistentExactTypeId,
        interface: PersistentExactTypeId,
    },
    DuplicateOwner {
        owner: PersistentExactTypeId,
    },
    NonCanonicalOwnerOrder {
        index: usize,
    },
    NonCanonicalInterfaceOrder {
        index: usize,
    },
    DuplicateSlot {
        slot: PersistentDispatchSlotId,
    },
    Position {
        index: usize,
    },
    OwnerKind {
        owner: PersistentExactTypeId,
    },
    SlotRole {
        slot: PersistentDispatchSlotId,
    },
    SlotSignature {
        slot: PersistentDispatchSlotId,
    },
    TargetSignature {
        slot: PersistentDispatchSlotId,
    },
    InvalidImplementation {
        slot: PersistentDispatchSlotId,
    },
    ConcreteObligation {
        owner: PersistentExactTypeId,
        slot: PersistentDispatchSlotId,
    },
    InheritanceCycle {
        exact: PersistentExactTypeId,
    },
    MissingReceiverPath {
        owner: PersistentExactTypeId,
        receiver: PersistentExactTypeId,
    },
    UnrelatedSlotOwner {
        slot: PersistentDispatchSlotId,
    },
    InterfaceClosure {
        owner: PersistentExactTypeId,
    },
    BasePrefix {
        owner: PersistentExactTypeId,
    },
    InterfaceOrder {
        owner: PersistentExactTypeId,
        interface: PersistentExactTypeId,
    },
}
impl From<IdentityReferenceError> for MirDispatchSchemaError {
    fn from(error: IdentityReferenceError) -> Self {
        Self::Reference(error)
    }
}
impl From<WireError> for MirDispatchSchemaError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<MirTypeBridgeLookupError> for MirDispatchSchemaError {
    fn from(error: MirTypeBridgeLookupError) -> Self {
        Self::Lookup(error)
    }
}
impl std::fmt::Display for MirDispatchSchemaError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "MIR dispatch schema: {self:?}")
    }
}
impl std::error::Error for MirDispatchSchemaError {}

impl From<scoop_identity::CallableBodyResolutionError<IdentityReferenceError>>
    for MirDispatchSchemaError
{
    fn from(error: scoop_identity::CallableBodyResolutionError<IdentityReferenceError>) -> Self {
        Self::DefinitionReference(error)
    }
}
