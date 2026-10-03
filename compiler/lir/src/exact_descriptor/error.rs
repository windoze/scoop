use scoop_identity::{
    PersistentDispatchTableId, PersistentExactTypeId, PersistentLayoutId, PersistentScanId,
};
use scoop_wire::WireError;

#[derive(Debug)]
pub enum ExactDescriptorError {
    CountOverflow,
    Provider,
    Target,
    MissingValueLayout(PersistentExactTypeId),
    MissingInstanceLayout(PersistentExactTypeId),
    LayoutRole(PersistentLayoutId),
    LayoutExact(PersistentExactTypeId),
    InstanceLayout(PersistentLayoutId),
    InstanceScan(PersistentScanId),
    InstanceShape(PersistentExactTypeId),
    InlineScan(PersistentExactTypeId),
    MissingInlineLayout(PersistentLayoutId),
    DispatchTable(PersistentDispatchTableId),
    DispatchInventory(PersistentExactTypeId),
    NonCanonicalInterfaces(PersistentExactTypeId),
    DuplicateDispatchTable(PersistentDispatchTableId),
    DuplicateInterface(PersistentExactTypeId),
    DiagnosticName(PersistentExactTypeId),
    Diagnostic(scoop_identity::ExactTypeDiagnosticError),
    Definition(crate::StrongShapeDefinitionError),
    DefinitionSubject,
    DescriptorPlan(PersistentExactTypeId),
    LayoutPlan(PersistentExactTypeId),
    InlineScanPlan(PersistentExactTypeId),
    DiagnosticAtom(PersistentExactTypeId),
    ItableDirectory(PersistentExactTypeId),
    RegistrationDefinition(PersistentExactTypeId),
    RegistrationSymbol(PersistentExactTypeId),
    RegistrationExact(PersistentExactTypeId),
    RegistrationFingerprint(PersistentExactTypeId),
    RegistrationFingerprintHash(scoop_wire::HashError),
    IdentityHash(scoop_wire::HashError),
    Resource(WireError),
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ExactDescriptorError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

from_error!(scoop_identity::ExactTypeDiagnosticError, Diagnostic);
from_error!(crate::StrongShapeDefinitionError, Definition);
from_error!(WireError, Resource);
from_error!(scoop_wire::HashError, IdentityHash);

impl std::fmt::Display for ExactDescriptorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid exact TypeDescriptor export: {self:?}")
    }
}

impl std::error::Error for ExactDescriptorError {}
