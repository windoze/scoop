use super::*;

#[derive(Debug)]
pub enum LayoutAbiSectionError<E> {
    Source(E),
    DuplicateProvider(ConeIdentity),
    DependencyTarget { provider: ConeIdentity },
    SelectedClosure,
    NonCanonicalSelected { index: usize },
    SelectedCurrentProvider,
    SelectionIdentityExhausted,
    MissingSemanticProvider(ConeIdentity),
    MissingPhysicalProvider(ConeIdentity),
    MissingPhysicalSubject(crate::ExternalStrongShapeSubjectV1),
    MissingPhysicalSemantic(LayoutAbiDependencyV1),
    Exports(LayoutAbiExportConstituentsError),
    Semantic(LayoutAbiSemanticClosureError),
    Layout(crate::ExactLayoutTableError),
    Descriptor(crate::ExactDescriptorTableError),
    Dispatch(crate::ExactDispatchTableError),
    DescriptorDispatch(scoop_identity::PersistentDispatchTableId),
    Callable(crate::ExactCallableAbiTableError),
    ShapeSupport,
    Physical(crate::ShapeLinkError),
    Dependency(LayoutAbiDependencyError),
    Identity(IdentityReferenceError),
    Encoding(scoop_wire::cbor::EncodeError),
    Resource(WireError),
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl<E> From<$source> for LayoutAbiSectionError<E> {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

from_error!(LayoutAbiExportConstituentsError, Exports);
from_error!(LayoutAbiSemanticClosureError, Semantic);
from_error!(crate::ExactLayoutTableError, Layout);
from_error!(crate::ExactDescriptorTableError, Descriptor);
from_error!(crate::ExactDispatchTableError, Dispatch);
from_error!(crate::ExactCallableAbiTableError, Callable);
from_error!(crate::ShapeLinkError, Physical);
from_error!(LayoutAbiDependencyError, Dependency);
from_error!(IdentityReferenceError, Identity);
from_error!(scoop_wire::cbor::EncodeError, Encoding);
from_error!(WireError, Resource);

impl<E: std::fmt::Debug> std::fmt::Display for LayoutAbiSectionError<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid complete layout/ABI section: {self:?}")
    }
}

impl<E: std::fmt::Debug> std::error::Error for LayoutAbiSectionError<E> {}
