use scoop_wire::{Encoder, WireEncode};

use super::*;

mod instance;
mod read;
mod value;
pub use read::{
    DecodedExactLayoutExportV1, DecodedExactLayoutSemanticProjectionV1, ExactLayoutWireError,
};

type EncodeResult = Result<(), scoop_wire::cbor::EncodeError>;

impl WireEncode for ExactLayoutExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(7)?;
        self.encode_semantic_fields(encoder)?;
        field(encoder, 7, &self.identity().definition())
    }
}

fn field(encoder: &mut Encoder, index: u32, value: &impl WireEncode) -> EncodeResult {
    encoder.field(index)?;
    value.encode(encoder)
}

fn unsigned(encoder: &mut Encoder, index: u32, value: u64) -> EncodeResult {
    encoder.field(index)?;
    encoder.unsigned(value)
}

fn sum(encoder: &mut Encoder, tag: u64, payload_fields: u64) -> EncodeResult {
    encoder.map(payload_fields + 1)?;
    unsigned(encoder, 0, tag)
}

fn array<T: WireEncode>(encoder: &mut Encoder, values: &[T]) -> EncodeResult {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn nominal_fields(encoder: &mut Encoder, fields: &[crate::PlacedFieldStorageV1]) -> EncodeResult {
    encoder.array(fields.len() as u64)?;
    for item in fields {
        encoder.map(3)?;
        field(encoder, 1, &item.field())?;
        field(encoder, 2, item.storage())?;
        unsigned(encoder, 3, item.access_alignment().get())?;
    }
    Ok(())
}

fn pointer(encoder: &mut Encoder, kind: crate::NullNicheKind) -> EncodeResult {
    use crate::NullNicheKind::*;
    sum(
        encoder,
        match kind {
            Managed => 1,
            Raw => 2,
            Code => 3,
            Interface => 4,
        },
        0,
    )
}

/// A borrowed semantic projection; no definition or import authority is added.
#[derive(Clone, Copy, Debug)]
pub struct ExactLayoutSemanticProjectionV1<'a>(&'a ExactLayoutExportV1);

impl ExactLayoutExportV1 {
    pub const fn semantic_projection(&self) -> ExactLayoutSemanticProjectionV1<'_> {
        ExactLayoutSemanticProjectionV1(self)
    }

    fn encode_semantic_fields(&self, encoder: &mut Encoder) -> EncodeResult {
        let identity = self.identity();
        field(encoder, 1, &identity.layout())?;
        field(encoder, 2, &identity.exact())?;
        field(encoder, 3, &identity.target().wire_id())?;
        field(encoder, 4, &identity.layout_key().representation())?;
        encoder.field(5)?;
        match self.kind() {
            ExactLayoutBodyKindV1::Value(value) => {
                sum(encoder, 1, 2)?;
                field(encoder, 1, value.value().storage())?;
                field(encoder, 2, value.representation())?;
            }
            ExactLayoutBodyKindV1::Instance(instance) => {
                sum(encoder, 2, 2)?;
                field(encoder, 1, instance.shape())?;
                field(encoder, 2, instance.representation())?;
            }
        }
        field(encoder, 6, &self.scan())
    }
}

impl WireEncode for ExactLayoutSemanticProjectionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(6)?;
        self.0.encode_semantic_fields(encoder)
    }
}
