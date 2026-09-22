use super::*;

#[derive(Debug)]
pub enum LayoutAbiExportLoweringError {
    Provider,
    Target,
    CountOverflow,
    DuplicateProvider(ConeIdentity),
    MissingLayout(PersistentExactTypeId),
    AmbiguousLayout(PersistentExactTypeId),
    MissingCallable(StrongCallableDefinitionOwner),
    AmbiguousCallable(StrongCallableDefinitionOwner),
    MissingType(PersistentExactTypeId),
    MissingDispatch(PersistentExactTypeId),
    MissingInterface(PersistentDispatchTableId),
    DescriptorSet,
    DescriptorProduction(PersistentExactTypeId),
    Layout(crate::ExactLayoutLoweringError),
    Callable {
        target: StrongCallableDefinitionOwner,
        source: crate::ExactCallableAbiLoweringError,
    },
    CallableTable(lir::ExactCallableAbiTableError),
    Descriptor(lir::ExactDescriptorError),
    DescriptorTable(lir::ExactDescriptorTableError),
    Dispatch(lir::ExactDispatchError),
    DispatchTable(lir::ExactDispatchTableError),
    Shape(lir::ParamFreeShapeSupportTableError),
    Exports(lir::LayoutAbiExportConstituentsError),
    Diagnostic(scoop_identity::ExactTypeDiagnosticCatalogError),
    Encoding(scoop_wire::cbor::EncodeError),
    Resource(WireError),
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for LayoutAbiExportLoweringError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(crate::ExactLayoutLoweringError, Layout);
from_error!(lir::ExactCallableAbiTableError, CallableTable);
from_error!(lir::ExactDescriptorError, Descriptor);
from_error!(lir::ExactDescriptorTableError, DescriptorTable);
from_error!(lir::ExactDispatchError, Dispatch);
from_error!(lir::ExactDispatchTableError, DispatchTable);
from_error!(lir::ParamFreeShapeSupportTableError, Shape);
from_error!(lir::LayoutAbiExportConstituentsError, Exports);
from_error!(scoop_identity::ExactTypeDiagnosticCatalogError, Diagnostic);
from_error!(scoop_wire::cbor::EncodeError, Encoding);
from_error!(WireError, Resource);

impl std::fmt::Display for LayoutAbiExportLoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot assemble LIR layout/ABI exports: {self:?}")
    }
}
impl std::error::Error for LayoutAbiExportLoweringError {}
