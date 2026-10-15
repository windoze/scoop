use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirTypeBridgeUnitProblemV1 {
    MissingRole,
    DefinitionRole,
    Signature,
}

#[derive(Debug)]
pub enum MirTypeBridgeSectionError {
    Resource(WireError),
    Identity(IdentityReferenceError),
    Type(MirTypeBridgeError),
    Callable(MirCallableBridgeError),
    Dispatch(MirDispatchSchemaError),
    Object(MirObjectBridgeError),
    Shape(MirShapeSupportError),
    Lookup(MirTypeBridgeLookupError),
    References(MirTypeBridgeReferenceError),
    ProviderContext,
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
    SelectedCurrentProvider,
    ArithmeticOverflow,
}
macro_rules! from_error {
    ($error:ty, $variant:ident) => {
        impl From<$error> for MirTypeBridgeSectionError {
            fn from(value: $error) -> Self {
                Self::$variant(value)
            }
        }
    };
}
from_error!(WireError, Resource);
from_error!(IdentityReferenceError, Identity);
from_error!(MirTypeBridgeError, Type);
from_error!(MirCallableBridgeError, Callable);
from_error!(MirDispatchSchemaError, Dispatch);
from_error!(MirObjectBridgeError, Object);
from_error!(MirShapeSupportError, Shape);
from_error!(MirTypeBridgeLookupError, Lookup);
from_error!(MirTypeBridgeReferenceError, References);
impl std::fmt::Display for MirTypeBridgeSectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "MIR type bridge section: {self:?}")
    }
}
impl std::error::Error for MirTypeBridgeSectionError {}
