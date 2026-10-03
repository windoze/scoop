use super::*;
use scoop_wire::{Encoder, WireEncode};

impl WireEncode for SelectedExternalTypeUseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.provider.encode(encoder)?;
        encoder.field(2)?;
        self.usage.encode(encoder)
    }
}
impl WireEncode for SelectedTypeUseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Signature { exact } => payload(encoder, 1, exact, None),
            Self::Representation { exact } => payload(encoder, 2, exact, None),
            Self::Construct { exact, declaration } => payload(encoder, 3, exact, Some(declaration)),
            Self::MemberCall {
                receiver,
                declaration,
            } => payload(encoder, 4, receiver, Some(declaration)),
            Self::SlotCall { receiver, slot } => payload(encoder, 5, receiver, Some(slot)),
            Self::TypeTest { exact } => payload(encoder, 6, exact, None),
            Self::SingletonValue { exact, value } => payload(encoder, 7, exact, Some(value)),
            Self::Inheritance { derived, edge } => payload(encoder, 8, derived, Some(edge)),
            Self::ShapeSupport { exact } => payload(encoder, 9, exact, None),
        }
    }
}
impl WireEncode for SelectedTypeConstructionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Constructor(id) => payload(encoder, 1, id, None),
            Self::EnumVariant(id) => payload(encoder, 2, id, None),
        }
    }
}
impl WireEncode for SelectedDirectInheritanceEdgeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ClassBase { exact } => payload(encoder, 1, exact, None),
            Self::Interface { exact } => payload(encoder, 2, exact, None),
        }
    }
}
pub(super) fn payload(
    encoder: &mut Encoder,
    tag: u64,
    first: &dyn WireEncode,
    second: Option<&dyn WireEncode>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    wire::tag(encoder, if second.is_some() { 3 } else { 2 }, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    if let Some(second) = second {
        encoder.field(2)?;
        second.encode(encoder)?;
    }
    Ok(())
}
