use super::*;
use crate::{DeclarationAccessSourceV1, SourceNominalId};
use scoop_wire::{Encoder, WireEncode};

mod decode;
mod errors;
mod references;
mod table;
#[cfg(test)]
mod tests;
mod validation;

pub use decode::*;
pub use errors::*;
pub use references::*;
pub use table::*;
pub use validation::*;

/// The protected declaration surface is distinct from nominal source support
/// and from the ordinary public lookup surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtectedDeclarationInterfaceV1 {
    Callable(Box<ProtectedCallableInterfaceV1>),
    Constructor(Box<ProtectedConstructorInterfaceV1>),
    Property(Box<ProtectedPropertyInterfaceV1>),
    NestedNominal(Box<ProtectedNestedNominalInterfaceV1>),
}
impl ProtectedDeclarationInterfaceV1 {
    pub fn reference(&self) -> ProtectedDeclarationRefV1 {
        match self {
            Self::Callable(value) => ProtectedDeclarationRefV1::Callable(
                ProtectedCallableDeclarationRefV1(value.declaration()),
            ),
            Self::Constructor(value) => ProtectedDeclarationRefV1::Constructor(value.declaration()),
            Self::Property(value) => ProtectedDeclarationRefV1::Property(value.declaration()),
            Self::NestedNominal(value) => {
                ProtectedDeclarationRefV1::NestedNominal(value.declaration())
            }
        }
    }
    pub fn declaration_access(&self) -> &DeclarationAccessSourceV1 {
        match self {
            Self::Callable(value) => value.declaration_access(),
            Self::Constructor(value) => value.declaration_access(),
            Self::Property(value) => value.declaration_access(),
            Self::NestedNominal(value) => value.declaration_access(),
        }
    }
}
impl WireEncode for ProtectedDeclarationInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Callable(value) => encode_record(
                encoder,
                1,
                &value.declaration(),
                value.declaration_access(),
                value.payload(),
            ),
            Self::Constructor(value) => encode_record(
                encoder,
                2,
                &value.declaration(),
                value.declaration_access(),
                value.payload(),
            ),
            Self::Property(value) => encode_record(
                encoder,
                3,
                &value.declaration(),
                value.declaration_access(),
                value.payload(),
            ),
            Self::NestedNominal(value) => encode_record(
                encoder,
                4,
                &value.declaration(),
                value.declaration_access(),
                value.payload(),
            ),
        }
    }
}
fn encode_record(
    encoder: &mut Encoder,
    tag: u64,
    declaration: &dyn WireEncode,
    access: &DeclarationAccessSourceV1,
    payload: &dyn WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    wire::tag(encoder, 4, tag)?;
    encoder.field(1)?;
    declaration.encode(encoder)?;
    encoder.field(2)?;
    access.encode(encoder)?;
    encoder.field(3)?;
    payload.encode(encoder)
}
