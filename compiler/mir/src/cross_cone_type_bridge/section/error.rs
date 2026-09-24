use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirTypeBridgeUnitProblemV1 {
    WrongProvider,
    MissingRole,
    MissingSource,
    GenericSource,
    DefinitionRole,
    Signature,
    ProducerInventory,
    ProducerRole,
}

#[derive(Debug)]
pub enum MirTypeBridgeSectionError<E> {
    Resource(WireError),
    Identity(IdentityReferenceError),
    GeneratedIdentity(scoop_identity::GeneratedCallableIdentityError),
    Encoding(scoop_wire::cbor::EncodeError),
    Source(E),
    SourceJoin(MirTypeBridgeSourceJoinError<E>),
    Type(MirTypeBridgeError),
    Callable(MirCallableBridgeError),
    Dispatch(MirDispatchSchemaError),
    Object(MirObjectBridgeError),
    Shape(MirShapeSupportError),
    Lookup(MirTypeBridgeLookupError),
    References(MirTypeBridgeReferenceError),
    ProviderContext,
    FoundationSurface,
    NonCanonicalUnitInventory,
    NonCanonicalCommittedUses,
    NonCanonicalSelected {
        index: usize,
    },
    Unit {
        unit: PersistentInitializationUnitId,
        problem: MirTypeBridgeUnitProblemV1,
    },
    DuplicateProvider(ConeIdentity),
    DuplicateTarget(MirTypeBridgeTargetV1),
    MissingTarget(MirTypeBridgeDependencyV1),
    MissingDependency(MirTypeBridgeTargetV1),
    OldCallablePartition(StrongCallableDefinitionOwner),
    SelectedCurrentProvider,
    SelectedClosure,
    SelectionIdentityExhausted,
    ArithmeticOverflow,
}
macro_rules! from_error {
    ($error:ty, $variant:ident) => {
        impl<E> From<$error> for MirTypeBridgeSectionError<E> {
            fn from(value: $error) -> Self {
                Self::$variant(value)
            }
        }
    };
}
from_error!(WireError, Resource);
from_error!(IdentityReferenceError, Identity);
from_error!(
    scoop_identity::GeneratedCallableIdentityError,
    GeneratedIdentity
);
from_error!(MirTypeBridgeSourceJoinError<E>, SourceJoin);
from_error!(MirTypeBridgeError, Type);
from_error!(MirCallableBridgeError, Callable);
from_error!(MirDispatchSchemaError, Dispatch);
from_error!(MirObjectBridgeError, Object);
from_error!(MirShapeSupportError, Shape);
from_error!(MirTypeBridgeLookupError, Lookup);
from_error!(MirTypeBridgeReferenceError, References);
impl<E: std::fmt::Debug> std::fmt::Display for MirTypeBridgeSectionError<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "MIR type bridge section: {self:?}")
    }
}
impl<E: std::fmt::Debug> std::error::Error for MirTypeBridgeSectionError<E> {}
