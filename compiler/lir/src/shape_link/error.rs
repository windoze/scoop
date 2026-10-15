use scoop_identity::{ConeIdentity, PersistentSymbolRequest};
use scoop_wire::WireError;

use crate::ExternalStrongShapeSubjectV1;

#[derive(Debug)]
pub enum ShapeLinkError {
    Provider,
    Target,
    LocalImport,
    Identity(scoop_identity::IdentityReferenceError),
    Symbol(scoop_identity::PersistentSymbolResolutionError<scoop_identity::IdentityReferenceError>),
    Subject(crate::ExternalShapeSubjectResolutionError),
    MissingProvider(ConeIdentity),
    MissingSubject(ExternalStrongShapeSubjectV1),
    SupportRelation(ExternalStrongShapeSubjectV1),
    DefinitionRelation(ExternalStrongShapeSubjectV1),
    ConsumerDefinition(PersistentSymbolRequest),
    Duplicate {
        provider: ConeIdentity,
        subject: ExternalStrongShapeSubjectV1,
    },
    Order,
    Definition(crate::StrongShapeDefinitionError),
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
from_error!(scoop_identity::IdentityReferenceError, Identity);
from_error!(crate::ExternalShapeSubjectResolutionError, Subject);
from_error!(crate::StrongShapeDefinitionError, Definition);

impl std::fmt::Display for ShapeLinkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid external shape Link semantics: {self:?}")
    }
}
impl std::error::Error for ShapeLinkError {}

from_error!(
    scoop_identity::PersistentSymbolResolutionError<scoop_identity::IdentityReferenceError>,
    Symbol
);
