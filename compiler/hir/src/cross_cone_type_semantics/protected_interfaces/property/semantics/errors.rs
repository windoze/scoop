use crate::{AccessDomainSemanticError, InheritanceGraphError, MeteredSignatureTypeSemanticError};
use scoop_wire::WireError;
use std::fmt;

#[derive(Debug)]
pub enum ProtectedPropertySemanticError<E> {
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    Foundation(E),
    Signature(MeteredSignatureTypeSemanticError<E>),
    Source(InheritanceGraphError<E>),
    Domain(AccessDomainSemanticError),
    Owner,
    Identity,
    ValueType,
    SourceShape,
    Accessor,
    SetterDomain,
}
impl<E: fmt::Display> fmt::Display for ProtectedPropertySemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Signature(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::Domain(error) => error.fmt(f),
            Self::Owner => {
                f.write_str("protected property requires a canonical lexical class owner")
            }
            Self::Identity => {
                f.write_str("protected property source key disagrees with its identity or owner")
            }
            Self::ValueType => {
                f.write_str("protected property value type differs from its logical source type")
            }
            Self::SourceShape => {
                f.write_str("protected property capability or representation differs from source")
            }
            Self::Accessor => {
                f.write_str("protected property accessor key names another property or role")
            }
            Self::SetterDomain => {
                f.write_str("setter effective access domain is wider than its property")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for ProtectedPropertySemanticError<E> {}

#[derive(Debug)]
pub enum ProtectedPropertyAccessorClosureError {
    Resource(WireError),
    Encoding(scoop_wire::cbor::EncodeError),
    Getter,
    Setter,
    Signature,
    Access,
    Representation,
    Slot,
}
impl fmt::Display for ProtectedPropertyAccessorClosureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Resource(error) => return error.fmt(f),
            Self::Encoding(error) => return error.fmt(f),
            Self::Getter => "protected property is missing its checked getter source",
            Self::Setter => "protected property setter closure disagrees with its visibility",
            Self::Signature => {
                "protected property accessor signature differs from its logical type"
            }
            Self::Access => "protected property accessor has another owner or access source",
            Self::Representation => "property representation and accessor source modality disagree",
            Self::Slot => "protected property omits an accessor slot relation",
        })
    }
}
impl std::error::Error for ProtectedPropertyAccessorClosureError {}
