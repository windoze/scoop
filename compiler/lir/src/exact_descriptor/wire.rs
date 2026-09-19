use scoop_wire::{Encoder, WireEncode};

use super::*;

mod read;
pub use read::{
    DecodedCanonicalExactDescriptorExportsV1, DecodedExactDescriptorExportV1,
    ExactDescriptorWireError,
};

impl WireEncode for ExactDescriptorExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        field(encoder, 1, &self.exact())?;
        field(encoder, 2, &self.value_layout().identity().layout())?;
        field(encoder, 3, &self.instance_layout().identity().layout())?;
        field(encoder, 4, self.shape())?;
        encoder.field(5)?;
        crate::scan::encode_canonical_scan(self.object_scan(), encoder)?;
        field(encoder, 6, self.ancestry())?;
        field(encoder, 7, self.dispatch())?;
        encoder.field(8)?;
        encoder.text(self.diagnostic_name().as_str())?;
        field(encoder, 9, &self.definition())?;
        field(encoder, 10, &self.registration())
    }
}

impl WireEncode for ExactDescriptorAncestryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        optional_descriptor(self.parent(), encoder)?;
        encoder.field(2)?;
        sequence(encoder, self.interfaces())
    }
}

impl WireEncode for ExactDescriptorDispatchV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        field(encoder, 1, &self.vtable())?;
        field(encoder, 2, &ItableSequence(self.itables()))
    }
}

impl WireEncode for ExactDescriptorItableV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        field(encoder, 1, &self.interface())?;
        field(encoder, 2, &self.table())
    }
}

impl WireEncode for CanonicalExactDescriptorExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, self.records())
    }
}

struct ItableSequence<'a>(&'a [ExactDescriptorItableV1]);
impl WireEncode for ItableSequence<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, self.0)
    }
}

fn optional_descriptor(
    value: Option<crate::StrongTypeDescriptorRefV2>,
    encoder: &mut Encoder,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    use crate::OptionalStrongTypeDescriptorRefV2 as Optional;
    match value {
        None => Optional::Absent,
        Some(crate::StrongTypeDescriptorRefV2::Local(exact)) => Optional::Local(exact),
        Some(crate::StrongTypeDescriptorRefV2::CoreExternal(exact)) => {
            Optional::CoreExternal(exact)
        }
        Some(crate::StrongTypeDescriptorRefV2::DependencyExternal { provider, exact }) => {
            Optional::DependencyExternal { provider, exact }
        }
    }
    .encode(encoder)
}

fn field(
    encoder: &mut Encoder,
    index: u32,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(index)?;
    value.encode(encoder)
}

fn sequence<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}
