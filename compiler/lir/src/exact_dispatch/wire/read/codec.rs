use scoop_identity::{
    DecodedCallableDefinitionOwner, DecodedDispatchDeclarationOwner, DecodedExactCallableSignature,
    DecodedPersistentId, GcEffect,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::*;

impl WireDecode for DecodedExactDispatchExportV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            semantic: DecodedExactDispatchSemanticProjectionV1::decode_fields(decoder)?,
            definition: decoder.field(5, DecodedStrongShapeDefinitionV1::decode)?,
        })
    }
}

impl WireDecode for DecodedCanonicalExactDispatchExportsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExactDispatchExportV1::decode(decoder))
            .map(|records| Self { records })
    }
}

impl WireDecode for DecodedExactDispatchRoleV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match (fields, tag) {
            (1, 1) => Ok(Self::Vtable),
            (2, 2) => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Itable),
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for DecodedExactDispatchEntryV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        Ok(Self {
            position: decoder.field(1, Decoder::u32)?,
            slot: decoder.field(2, DecodedPersistentId::decode)?,
            slot_signature: decoder.field(3, DecodedExactDispatchSlotSignatureV1::decode)?,
            implementation: decoder.field(4, DecodedExactDispatchImplementationV1::decode)?,
            abi: decoder.field(5, DecodedStrongTypeDispatchCallableRefV2::decode)?,
        })
    }
}

impl WireDecode for DecodedExactDispatchSlotSignatureV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            exact: decoder.field(1, DecodedExactCallableSignature::decode)?,
            gc_effect: decoder.field(2, GcEffect::decode)?,
        })
    }
}

impl WireDecode for DecodedExactDispatchReceiverAdaptationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(1)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::Identity),
            2 => Ok(Self::ReferenceDispatch),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for DecodedExactDispatchImplementationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match (fields, tag) {
            (4, 1) => Ok(Self::AbstractObligation {
                declaration: decoder.field(1, DecodedDispatchDeclarationOwner::decode)?,
                trap_target: decoder.field(2, DecodedCallableDefinitionOwner::decode)?,
                receiver: decoder.field(3, DecodedExactDispatchReceiverAdaptationV1::decode)?,
            }),
            (3, 2) => Ok(Self::DirectStrongTarget {
                target: decoder.field(1, DecodedCallableDefinitionOwner::decode)?,
                receiver: decoder.field(2, DecodedExactDispatchReceiverAdaptationV1::decode)?,
            }),
            (3, 3) => Ok(Self::InterfaceDefaultTarget {
                target: decoder.field(1, DecodedCallableDefinitionOwner::decode)?,
                receiver: decoder.field(2, DecodedExactDispatchReceiverAdaptationV1::decode)?,
            }),
            (2, 4) => decoder
                .field(1, DecodedCallableDefinitionOwner::decode)
                .map(Self::AdjustThunkTarget),
            _ => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireEncode for DecodedExactDispatchExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        self.semantic.encode_fields(encoder)?;
        encoder.field(5)?;
        self.definition.encode(encoder)
    }
}

impl WireEncode for DecodedCanonicalExactDispatchExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_array(encoder, &self.records)
    }
}

impl WireEncode for DecodedExactDispatchRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Vtable => encode_empty_sum(encoder, 1),
            Self::Itable(interface) => encode_value_sum(encoder, 2, interface),
        }
    }
}

impl WireEncode for DecodedExactDispatchEntryV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        encoder.unsigned(u64::from(self.position))?;
        encoder.field(2)?;
        self.slot.encode(encoder)?;
        encoder.field(3)?;
        self.slot_signature.encode(encoder)?;
        encoder.field(4)?;
        self.implementation.encode(encoder)?;
        encoder.field(5)?;
        self.abi.encode(encoder)
    }
}

impl WireEncode for DecodedExactDispatchSlotSignatureV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.exact.encode(encoder)?;
        encoder.field(2)?;
        self.gc_effect.encode(encoder)
    }
}

impl WireEncode for DecodedExactDispatchReceiverAdaptationV1 {
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

impl WireEncode for DecodedExactDispatchImplementationV1 {
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

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

impl DecodedExactDispatchSemanticProjectionV1 {
    fn decode_fields(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        Ok(Self {
            table: decoder.field(1, DecodedPersistentId::decode)?,
            owner_exact: decoder.field(2, DecodedPersistentId::decode)?,
            role: decoder.field(3, DecodedExactDispatchRoleV1::decode)?,
            entries: decoder.field(4, |decoder| {
                decoder.decode_array(|decoder, _| DecodedExactDispatchEntryV1::decode(decoder))
            })?,
        })
    }
    fn encode_fields(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.field(1)?;
        self.table.encode(encoder)?;
        encoder.field(2)?;
        self.owner_exact.encode(encoder)?;
        encoder.field(3)?;
        self.role.encode(encoder)?;
        encoder.field(4)?;
        encode_array(encoder, &self.entries)
    }
}
impl WireDecode for DecodedExactDispatchSemanticProjectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Self::decode_fields(decoder)
    }
}
impl WireEncode for DecodedExactDispatchSemanticProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        self.encode_fields(encoder)
    }
}
