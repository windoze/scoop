use super::*;

#[derive(Debug)]
pub(super) enum RawInstance {
    Class {
        base: RawBase,
        declared: Vec<RawNominalField>,
        complete: Vec<RawNominalField>,
    },
    Box {
        exact: DecodedPersistentId<PersistentExactTypeId>,
        layout: DecodedPersistentId<PersistentLayoutId>,
    },
    InlineBytes,
    InlineArray {
        exact: DecodedPersistentId<PersistentExactTypeId>,
        storage: crate::DecodedArrayElementStorageV1,
    },
    AbstractReference,
}

#[derive(Debug)]
pub(super) enum RawBase {
    NoBase,
    Prefix {
        exact: DecodedPersistentId<PersistentExactTypeId>,
        layout: DecodedPersistentId<PersistentLayoutId>,
        size: u64,
        alignment: u64,
    },
}

impl WireDecode for RawInstance {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                length(decoder, fields, 4)?;
                Ok(Self::Class {
                    base: decoder.field(1, RawBase::decode)?,
                    declared: table(decoder, 2)?,
                    complete: table(decoder, 3)?,
                })
            }
            2 => {
                length(decoder, fields, 3)?;
                Ok(Self::Box {
                    exact: decoder.field(1, DecodedPersistentId::decode)?,
                    layout: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            3 => {
                length(decoder, fields, 1)?;
                Ok(Self::InlineBytes)
            }
            4 => {
                length(decoder, fields, 3)?;
                Ok(Self::InlineArray {
                    exact: decoder.field(1, DecodedPersistentId::decode)?,
                    storage: decoder.field(2, crate::DecodedArrayElementStorageV1::decode)?,
                })
            }
            5 => {
                length(decoder, fields, 1)?;
                Ok(Self::AbstractReference)
            }
            tag => Err(unknown(decoder, tag)),
        }
    }
}

impl WireDecode for RawBase {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                length(decoder, fields, 1)?;
                Ok(Self::NoBase)
            }
            2 => {
                length(decoder, fields, 5)?;
                Ok(Self::Prefix {
                    exact: decoder.field(1, DecodedPersistentId::decode)?,
                    layout: decoder.field(2, DecodedPersistentId::decode)?,
                    size: decoder.field(3, Decoder::unsigned)?,
                    alignment: decoder.field(4, Decoder::unsigned)?,
                })
            }
            tag => Err(unknown(decoder, tag)),
        }
    }
}

impl WireEncode for RawInstance {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        match self {
            Self::Class {
                base,
                declared,
                complete,
            } => {
                sum(encoder, 1, 3)?;
                field(encoder, 1, base)?;
                encoder.field(2)?;
                array(encoder, declared)?;
                encoder.field(3)?;
                array(encoder, complete)
            }
            Self::Box { exact, layout } => {
                sum(encoder, 2, 2)?;
                field(encoder, 1, exact)?;
                field(encoder, 2, layout)
            }
            Self::InlineBytes => sum(encoder, 3, 0),
            Self::InlineArray { exact, storage } => {
                sum(encoder, 4, 2)?;
                field(encoder, 1, exact)?;
                field(encoder, 2, storage)
            }
            Self::AbstractReference => sum(encoder, 5, 0),
        }
    }
}

impl WireEncode for RawBase {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        match self {
            Self::NoBase => sum(encoder, 1, 0),
            Self::Prefix {
                exact,
                layout,
                size,
                alignment,
            } => {
                sum(encoder, 2, 4)?;
                field(encoder, 1, exact)?;
                field(encoder, 2, layout)?;
                unsigned(encoder, 3, *size)?;
                unsigned(encoder, 4, *alignment)
            }
        }
    }
}
