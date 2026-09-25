use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::{
    DecodedOptionalStrongTypeDescriptorRefV2, DecodedRefScanV1, DecodedStrongTypeDescriptorRefV2,
    DecodedTypeInstanceShapeV1,
};

impl WireDecode for DecodedExactDescriptorExportV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(10)?;
        Ok(Self {
            semantic: DecodedExactDescriptorSemanticProjectionV1::decode_fields(decoder)?,
            definition: decoder
                .field(9, crate::production::DecodedStrongShapeDefinitionV1::decode)?,
            registration: decoder.field(
                10,
                crate::production::DecodedStrongShapeRegistrationV1::decode,
            )?,
        })
    }
}

impl WireDecode for DecodedAncestry {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            parent: decoder.field(1, DecodedOptionalStrongTypeDescriptorRefV2::decode)?,
            interfaces: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedStrongTypeDescriptorRefV2::decode(decoder))
            })?,
        })
    }
}

impl WireDecode for DecodedDispatch {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            vtable: decoder.field(1, DecodedPersistentId::decode)?,
            itables: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedItable::decode(decoder))
            })?,
        })
    }
}

impl WireDecode for DecodedItable {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            interface: decoder.field(1, DecodedStrongTypeDescriptorRefV2::decode)?,
            table: decoder.field(2, DecodedPersistentId::decode)?,
        })
    }
}

impl WireDecode for DecodedCanonicalExactDescriptorExportsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedExactDescriptorExportV1::decode(decoder))
            .map(|records| Self { records })
    }
}

impl WireEncode for DecodedExactDescriptorExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        self.semantic.encode_fields(encoder)?;
        encoder.field(9)?;
        self.definition.encode(encoder)?;
        encoder.field(10)?;
        self.registration.encode(encoder)
    }
}

impl WireEncode for DecodedAncestry {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.parent.encode(encoder)?;
        encoder.field(2)?;
        encode_sequence(encoder, &self.interfaces)
    }
}

impl WireEncode for DecodedDispatch {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.vtable.encode(encoder)?;
        encoder.field(2)?;
        encode_sequence(encoder, &self.itables)
    }
}

impl WireEncode for DecodedItable {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.interface.encode(encoder)?;
        encoder.field(2)?;
        self.table.encode(encoder)
    }
}

impl WireEncode for DecodedCanonicalExactDescriptorExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_sequence(encoder, &self.records)
    }
}

fn encode_sequence<T: WireEncode>(
    encoder: &mut Encoder,
    records: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(records.len() as u64)?;
    for record in records {
        record.encode(encoder)?;
    }
    Ok(())
}

impl DecodedExactDescriptorSemanticProjectionV1 {
    fn decode_fields(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        Ok(Self {
            exact: decoder.field(1, DecodedPersistentId::decode)?,
            value_layout: decoder.field(2, DecodedPersistentId::decode)?,
            instance_layout: decoder.field(3, DecodedPersistentId::decode)?,
            shape: decoder.field(4, DecodedTypeInstanceShapeV1::decode)?,
            object_scan: decoder.field(5, DecodedRefScanV1::decode)?,
            ancestry: decoder.field(6, DecodedAncestry::decode)?,
            dispatch: decoder.field(7, DecodedDispatch::decode)?,
            diagnostic_name: decoder.field(8, Decoder::owned_text)?,
        })
    }
    fn encode_fields(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.field(1)?;
        self.exact.encode(encoder)?;
        encoder.field(2)?;
        self.value_layout.encode(encoder)?;
        encoder.field(3)?;
        self.instance_layout.encode(encoder)?;
        encoder.field(4)?;
        self.shape.encode(encoder)?;
        encoder.field(5)?;
        self.object_scan.encode(encoder)?;
        encoder.field(6)?;
        self.ancestry.encode(encoder)?;
        encoder.field(7)?;
        self.dispatch.encode(encoder)?;
        encoder.field(8)?;
        encoder.text(&self.diagnostic_name)
    }
}
impl WireDecode for DecodedExactDescriptorSemanticProjectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(8)?;
        Self::decode_fields(decoder)
    }
}
impl WireEncode for DecodedExactDescriptorSemanticProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        self.encode_fields(encoder)
    }
}
