use scoop_identity::{ConeIdentity, PersistentSymbolRequest};
use scoop_wire::WireError;

use crate::ExternalStrongShapeSubjectV1;

#[derive(Debug)]
pub enum ShapeLinkError {
    Provider,
    Target,
    LocalImport,
    MissingSubject(ExternalStrongShapeSubjectV1),
    LegacyPartition(ExternalStrongShapeSubjectV1),
    SupportRelation(ExternalStrongShapeSubjectV1),
    DefinitionRelation(ExternalStrongShapeSubjectV1),
    ConsumerDefinition(PersistentSymbolRequest),
    Duplicate {
        provider: ConeIdentity,
        subject: ExternalStrongShapeSubjectV1,
    },
    Count,
    Order,
    Header,
    Contract,
    Definition(crate::StrongShapeDefinitionError),
    Layout(crate::ExactLayoutWireError),
    Descriptor(crate::ExactDescriptorWireError),
    DescriptorPlan(crate::ExactDescriptorError),
    Dispatch(crate::ExactDispatchWireError),
    Scan(crate::MeteredScanValidationError),
    Storage(crate::StrongSemanticProjectionError),
    Resource(WireError),
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ShapeLinkError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(WireError, Resource);
from_error!(crate::StrongShapeDefinitionError, Definition);
from_error!(crate::ExactLayoutWireError, Layout);
from_error!(crate::ExactDescriptorWireError, Descriptor);
from_error!(crate::ExactDispatchWireError, Dispatch);
from_error!(crate::MeteredScanValidationError, Scan);
from_error!(crate::StrongSemanticProjectionError, Storage);

impl std::fmt::Display for ShapeLinkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid external shape Link semantics: {self:?}")
    }
}
impl std::error::Error for ShapeLinkError {}
