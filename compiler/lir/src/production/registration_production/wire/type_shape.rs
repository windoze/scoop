//! Untrusted type shape registration carriers.

use super::*;

#[derive(Debug)]
pub(in crate::production::registration_production) struct DecodedTypeInstanceShapeV1 {
    pub(in crate::production::registration_production) instance_kind: u32,
    pub(in crate::production::registration_production) inline_storage_kind: u32,
    pub(in crate::production::registration_production) minimum_size: u64,
    pub(in crate::production::registration_production) instance_alignment: u64,
    pub(in crate::production::registration_production) inline_offset: u64,
    pub(in crate::production::registration_production) inline_size: u64,
    pub(in crate::production::registration_production) inline_stride: u64,
    pub(in crate::production::registration_production) inline_alignment: u64,
    pub(in crate::production::registration_production) object_scan: DecodedRefScan,
    pub(in crate::production::registration_production) inline_scan: DecodedRefScan,
}

impl WireEncode for DecodedTypeInstanceShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(10)?;
        encode_unsigned_field(encoder, 1, u64::from(self.instance_kind))?;
        encode_unsigned_field(encoder, 2, u64::from(self.inline_storage_kind))?;
        encode_unsigned_field(encoder, 3, self.minimum_size)?;
        encode_unsigned_field(encoder, 4, self.instance_alignment)?;
        encode_unsigned_field(encoder, 5, self.inline_offset)?;
        encode_unsigned_field(encoder, 6, self.inline_size)?;
        encode_unsigned_field(encoder, 7, self.inline_stride)?;
        encode_unsigned_field(encoder, 8, self.inline_alignment)?;
        encode_field(encoder, 9, &self.object_scan)?;
        encode_field(encoder, 10, &self.inline_scan)
    }
}

impl WireDecode for DecodedTypeInstanceShapeV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(10)?;
        Ok(Self {
            instance_kind: decoder.field(1, Decoder::u32)?,
            inline_storage_kind: decoder.field(2, Decoder::u32)?,
            minimum_size: decoder.field(3, Decoder::unsigned)?,
            instance_alignment: decoder.field(4, Decoder::unsigned)?,
            inline_offset: decoder.field(5, Decoder::unsigned)?,
            inline_size: decoder.field(6, Decoder::unsigned)?,
            inline_stride: decoder.field(7, Decoder::unsigned)?,
            inline_alignment: decoder.field(8, Decoder::unsigned)?,
            object_scan: decoder.field(9, DecodedRefScan::decode)?,
            inline_scan: decoder.field(10, DecodedRefScan::decode)?,
        })
    }
}
