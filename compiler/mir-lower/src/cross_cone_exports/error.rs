use super::*;

#[derive(Debug)]
pub enum MirTypeBridgeExportProductionError {
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    ProviderMismatch,
    CountOverflow,
    Lookup(mir::MirTypeBridgeLookupError),
    Types(crate::SourceMirTypeProductionError),
    SourceCallables(crate::SourceMirCallableProductionError),
    Constructors(crate::SourceMirConstructorProductionError),
    Objects(mir::MirObjectProductionError),
    Boxing(mir::MirBoxingCallableProductionError),
    Equality(crate::SourceMirEqualityProductionError),
    Callables(mir::MirCallableBridgeError),
    Dispatch(crate::SourceMirDispatchProductionError),
    Shapes(mir::MirShapeSupportError),
    InitializationUse(mir::MirObjectBridgeError),
    MissingInitializationUnit(PersistentInitializationUnitId),
    OrdinaryCallableMismatch(StrongCallableDefinitionOwner),
    OrdinarySource(Box<hir::SharedTypeMetadataError>),
    OrdinaryClassification(hir::NominalCallableClassificationError),
    IncompleteOrdinaryCallables { expected: usize, actual: usize },
}

impl From<WireError> for Error {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot assemble MIR type bridge exports: {self:?}")
    }
}
impl std::error::Error for Error {}
