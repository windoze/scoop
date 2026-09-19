use scoop_identity::{
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId, PersistentId,
};

use super::*;

#[derive(Debug)]
pub(super) struct RawField<I: PersistentId> {
    pub(super) id: DecodedPersistentId<I>,
    pub(super) storage: crate::DecodedFieldStorageV1,
    pub(super) alignment: u64,
}

impl<I: PersistentId> WireDecode for RawField<I> {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            id: decoder.field(1, DecodedPersistentId::decode)?,
            storage: decoder.field(2, crate::DecodedFieldStorageV1::decode)?,
            alignment: decoder.field(3, Decoder::unsigned)?,
        })
    }
}
impl<I: PersistentId> WireEncode for RawField<I> {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(3)?;
        field(encoder, 1, &self.id)?;
        field(encoder, 2, &self.storage)?;
        unsigned(encoder, 3, self.alignment)
    }
}

pub(super) type RawNominalField = RawField<PersistentFieldId>;
pub(super) type RawVariantField = RawField<PersistentEnumVariantFieldId>;

#[derive(Debug)]
pub(super) struct RawVariant {
    pub(super) id: DecodedPersistentId<PersistentEnumVariantId>,
    pub(super) fields: Vec<RawVariantField>,
}
impl WireDecode for RawVariant {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            id: decoder.field(1, DecodedPersistentId::decode)?,
            fields: table(decoder, 2)?,
        })
    }
}
impl WireEncode for RawVariant {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(2)?;
        self.encode_fields(encoder)
    }
}
impl RawVariant {
    pub(super) fn encode_fields(&self, encoder: &mut Encoder) -> EncodeResult {
        field(encoder, 1, &self.id)?;
        encoder.field(2)?;
        array(encoder, &self.fields)
    }
}

#[derive(Debug)]
pub(super) struct RawRegion {
    pub(super) offset: u64,
    pub(super) size: u64,
    pub(super) alignment: u64,
}
impl WireDecode for RawRegion {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            offset: decoder.field(1, Decoder::unsigned)?,
            size: decoder.field(2, Decoder::unsigned)?,
            alignment: decoder.field(3, Decoder::unsigned)?,
        })
    }
}
impl WireEncode for RawRegion {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(3)?;
        unsigned(encoder, 1, self.offset)?;
        unsigned(encoder, 2, self.size)?;
        unsigned(encoder, 3, self.alignment)
    }
}

#[derive(Debug)]
pub(super) enum RawSlot {
    Shared { size: u64, alignment: u64 },
    Dedicated(RawRegion),
}
impl WireDecode for RawSlot {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                length(decoder, fields, 3)?;
                Ok(Self::Shared {
                    size: decoder.field(1, Decoder::unsigned)?,
                    alignment: decoder.field(2, Decoder::unsigned)?,
                })
            }
            2 => {
                length(decoder, fields, 4)?;
                Ok(Self::Dedicated(RawRegion {
                    offset: decoder.field(1, Decoder::unsigned)?,
                    size: decoder.field(2, Decoder::unsigned)?,
                    alignment: decoder.field(3, Decoder::unsigned)?,
                }))
            }
            tag => Err(unknown(decoder, tag)),
        }
    }
}
impl WireEncode for RawSlot {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        match self {
            Self::Shared { size, alignment } => {
                sum(encoder, 1, 2)?;
                unsigned(encoder, 1, *size)?;
                unsigned(encoder, 2, *alignment)
            }
            Self::Dedicated(region) => {
                sum(encoder, 2, 3)?;
                unsigned(encoder, 1, region.offset)?;
                unsigned(encoder, 2, region.size)?;
                unsigned(encoder, 3, region.alignment)
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct RawTaggedVariant {
    pub(super) variant: RawVariant,
    pub(super) slot: RawSlot,
}
impl WireDecode for RawTaggedVariant {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            variant: RawVariant {
                id: decoder.field(1, DecodedPersistentId::decode)?,
                fields: table(decoder, 2)?,
            },
            slot: decoder.field(3, RawSlot::decode)?,
        })
    }
}
impl WireEncode for RawTaggedVariant {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(3)?;
        self.variant.encode_fields(encoder)?;
        field(encoder, 3, &self.slot)
    }
}
