use super::*;
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug)]
pub enum TypeSelectionValidationError<E> {
    Resource(WireError),
    Source(E),
    Encoding(scoop_wire::cbor::EncodeError),
    MissingProvider(ConeIdentity),
    MissingTarget(SelectedExternalTypeUseV1),
    RequiresOdr(PersistentExactTypeId),
    GenericDefault,
    DefaultOrigin,
    LocalSupportOrigin,
    DeclarationOwner,
    DeclarationRole,
    Receiver,
    Slot,
    Object,
    DirectEdge,
    Inventory,
    Graph(InheritanceQueryError),
}
impl<E> From<WireError> for TypeSelectionValidationError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl<E: fmt::Display> fmt::Display for TypeSelectionValidationError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Source(e) => e.fmt(f),
            Self::Encoding(e) => e.fmt(f),
            Self::MissingProvider(provider) => write!(
                f,
                "selected type has no checked terminal provider {provider}"
            ),
            Self::MissingTarget(target) => write!(
                f,
                "selected type target is absent from its terminal section: {target:?}"
            ),
            Self::RequiresOdr(exact) => {
                write!(f, "selected type {exact} needs M23-7 ODR capability")
            }
            Self::GenericDefault => {
                f.write_str("generic source metadata cannot enter selected/default expansion")
            }
            Self::DefaultOrigin => {
                f.write_str("selected default provenance is outside checked source templates")
            }
            Self::LocalSupportOrigin => {
                f.write_str("selected local support provenance is outside actual local facts")
            }
            Self::DeclarationOwner => {
                f.write_str("selected declaration and terminal source owner disagree")
            }
            Self::DeclarationRole => {
                f.write_str("selected getter/setter declaration role disagrees")
            }
            Self::Receiver => {
                f.write_str("selected receiver is outside the declaration's checked ancestry")
            }
            Self::Slot => f.write_str(
                "selected slot has no matching terminal root contract and receiver schema",
            ),
            Self::Object => {
                f.write_str("selected singleton source object/value/backing relation disagrees")
            }
            Self::DirectEdge => {
                f.write_str("selected inheritance is not the actual direct source edge")
            }
            Self::Inventory => f.write_str(
                "selected table differs from all committed roots and recursive semantic edges",
            ),
            Self::Graph(e) => e.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TypeSelectionValidationError<E> {}
