use scoop_identity::{CLayoutOverride, CanonicalCAbiLayoutFingerprint, PersistentEnumVariantId};

use super::*;

mod encode;
mod scalar;

#[derive(Debug)]
pub(super) enum RawValue {
    Scalar(ScalarRepresentationKindV1),
    QualifiedPointer(crate::NullNicheKind),
    Struct {
        policy: RawPolicy,
        interior_mutable: bool,
        fields: Vec<RawNominalField>,
    },
    Tuple(Vec<crate::DecodedTupleElementStorageV1>),
    TaggedEnum {
        tag: RawRegion,
        pure: RawRegion,
        variants: Vec<RawTaggedVariant>,
    },
    NicheEnum {
        pointer: crate::NullNicheKind,
        variants: Vec<RawVariant>,
        payload: DecodedPersistentId<PersistentEnumVariantId>,
    },
    Unit,
    Interface,
    MaybeUninit(DecodedPersistentId<scoop_identity::PersistentLayoutId>),
}

#[derive(Debug)]
pub(super) enum RawPolicy {
    Ordinary,
    CLayout {
        aligned: CLayoutOverride,
        packed: CLayoutOverride,
        contract: DecodedPersistentId<CanonicalCAbiLayoutFingerprint>,
    },
}

impl WireDecode for RawValue {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                length(decoder, fields, 2)?;
                Ok(Self::Scalar(decoder.field(1, scalar::decode)?))
            }
            2 => {
                length(decoder, fields, 2)?;
                Ok(Self::QualifiedPointer(decoder.field(1, pointer_kind)?))
            }
            3 => {
                length(decoder, fields, 4)?;
                Ok(Self::Struct {
                    policy: decoder.field(1, RawPolicy::decode)?,
                    interior_mutable: decoder.field(2, bool_value)?,
                    fields: table(decoder, 3)?,
                })
            }
            4 => {
                length(decoder, fields, 2)?;
                Ok(Self::Tuple(table(decoder, 1)?))
            }
            5 => {
                length(decoder, fields, 4)?;
                Ok(Self::TaggedEnum {
                    tag: decoder.field(1, RawRegion::decode)?,
                    pure: decoder.field(2, RawRegion::decode)?,
                    variants: table(decoder, 3)?,
                })
            }
            6 => {
                length(decoder, fields, 4)?;
                Ok(Self::NicheEnum {
                    pointer: decoder.field(1, pointer_kind)?,
                    variants: table(decoder, 2)?,
                    payload: decoder.field(3, DecodedPersistentId::decode)?,
                })
            }
            7 => {
                length(decoder, fields, 2)?;
                decoder.field(1, |decoder| {
                    decoder.expect_map(1)?;
                    match decoder.field(0, Decoder::unsigned)? {
                        1 => Ok(Self::Unit),
                        tag => Err(unknown(decoder, tag)),
                    }
                })
            }
            8 => {
                length(decoder, fields, 1)?;
                Ok(Self::Interface)
            }
            9 => {
                length(decoder, fields, 2)?;
                Ok(Self::MaybeUninit(
                    decoder.field(1, DecodedPersistentId::decode)?,
                ))
            }
            tag => Err(unknown(decoder, tag)),
        }
    }
}

impl WireDecode for RawPolicy {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                length(decoder, fields, 1)?;
                Ok(Self::Ordinary)
            }
            2 => {
                length(decoder, fields, 4)?;
                Ok(Self::CLayout {
                    aligned: decoder.field(1, scalar::alignment)?,
                    packed: decoder.field(2, scalar::alignment)?,
                    contract: decoder.field(3, DecodedPersistentId::decode)?,
                })
            }
            tag => Err(unknown(decoder, tag)),
        }
    }
}
