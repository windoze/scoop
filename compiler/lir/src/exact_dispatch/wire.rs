use scoop_wire::{Encoder, WireEncode};

use super::*;

mod read;
pub use read::{
    DecodedCanonicalExactDispatchExportsV1, DecodedExactDispatchExportV1, ExactDispatchWireError,
};

impl WireEncode for ExactDispatchExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.table().encode(encoder)?;
        encoder.field(2)?;
        self.owner_exact().encode(encoder)?;
        encoder.field(3)?;
        self.role().encode(encoder)?;
        encoder.field(4)?;
        encode_array(encoder, self.entries())?;
        encoder.field(5)?;
        self.definition().encode(encoder)
    }
}

impl WireEncode for CanonicalExactDispatchExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, self.records())
    }
}

impl WireEncode for ExactDispatchRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Vtable => encode_empty_sum(encoder, 1),
            Self::Itable { interface_exact } => encode_value_sum(encoder, 2, interface_exact),
        }
    }
}

impl WireEncode for ExactDispatchEntryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.position().into_u32()))?;
        encoder.field(2)?;
        self.slot().encode(encoder)?;
        encoder.field(3)?;
        self.slot_signature().encode(encoder)?;
        encoder.field(4)?;
        self.implementation().encode(encoder)?;
        encoder.field(5)?;
        self.abi().encode(encoder)
    }
}

impl WireEncode for ExactDispatchSlotSignatureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.exact().encode(encoder)?;
        encoder.field(2)?;
        self.gc_effect().encode(encoder)
    }
}

impl WireEncode for ExactDispatchReceiverAdaptationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_empty_sum(
            encoder,
            match self {
                Self::Identity => 1,
                Self::ReferenceDispatch => 2,
            },
        )
    }
}

impl WireEncode for ExactDispatchImplementationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::AbstractObligation {
                declaration,
                trap_target,
                receiver,
            } => {
                encoder.map(4)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                declaration.encode(encoder)?;
                encoder.field(2)?;
                trap_target.encode(encoder)?;
                encoder.field(3)?;
                receiver.encode(encoder)
            }
            Self::DirectStrongTarget { target, receiver } => {
                encode_target_with_receiver(encoder, 2, target, receiver)
            }
            Self::InterfaceDefaultTarget { target, receiver } => {
                encode_target_with_receiver(encoder, 3, target, receiver)
            }
            Self::AdjustThunkTarget(target) => encode_value_sum(encoder, 4, target),
        }
    }
}

fn encode_target_with_receiver(
    encoder: &mut Encoder,
    tag: u64,
    target: &impl WireEncode,
    receiver: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    target.encode(encoder)?;
    encoder.field(2)?;
    receiver.encode(encoder)
}

fn encode_array<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}
