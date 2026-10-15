use scoop_wire::{Encoder, WireEncode};

pub(super) type EncodeResult = Result<(), scoop_wire::cbor::EncodeError>;
pub(super) fn field(encoder: &mut Encoder, index: u32, value: &impl WireEncode) -> EncodeResult {
    encoder.field(index)?;
    value.encode(encoder)
}
