use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::*;

/// Saved field offsets and typed references into already read value storage.
#[derive(Debug)]
pub struct DecodedFieldStorageV1(RawField);

#[derive(Debug)]
enum RawField {
    ElidedZst {
        exact: DecodedPersistentId<PersistentExactTypeId>,
        offset: u64,
        alignment: u64,
    },
    Stored {
        exact: DecodedPersistentId<PersistentExactTypeId>,
        offset: u64,
        layout: DecodedValueLayoutConstituentV1,
    },
}

impl DecodedFieldStorageV1 {
    pub(crate) fn read(
        self,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
        target: LirTargetProfile,
        extent: u64,
    ) -> Result<FieldStorageV1, crate::LinkDataError> {
        use crate::link_data::link_error;
        use scoop_identity::PersistentIdResolver;
        Ok(FieldStorageV1(match self.0 {
            RawField::ElidedZst {
                exact,
                offset,
                alignment,
            } => {
                if offset != 0 {
                    return Err(link_error("nonzero ZST field offset"));
                }
                FieldBody::ElidedZst {
                    exact: identities.resolve(exact).map_err(link_error)?,
                    alignment: NonZeroPow2::new(alignment).map_err(link_error)?,
                }
            }
            RawField::Stored {
                exact,
                offset,
                layout,
            } => {
                let exact = identities.resolve(exact).map_err(link_error)?;
                let layout = layout
                    .read(target, identities)?
                    .nonzero_ref()
                    .ok_or_else(|| link_error("missing nonzero field value layout"))?;
                if exact != layout.exact()
                    || offset
                        .checked_add(layout.storage().size().get())
                        .is_none_or(|end| end > extent)
                {
                    return Err(link_error("field reference or extent mismatch"));
                }
                FieldBody::Stored {
                    offset: ByteOffsetV1(offset),
                    layout,
                }
            }
        }))
    }
}

impl WireDecode for DecodedFieldStorageV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let field = match tag {
            1 => RawField::ElidedZst {
                exact: decoder.field(1, DecodedPersistentId::decode)?,
                offset: decoder.field(2, Decoder::unsigned)?,
                alignment: decoder.field(3, Decoder::unsigned)?,
            },
            2 => RawField::Stored {
                exact: decoder.field(1, DecodedPersistentId::decode)?,
                offset: decoder.field(2, Decoder::unsigned)?,
                layout: decoder.field(3, DecodedValueLayoutConstituentV1::decode)?,
            },
            _ => {
                return Err(WireError::new(
                    WireErrorKind::UnknownTag { tag },
                    decoder.path().clone(),
                    Some(decoder.position()),
                ));
            }
        };
        Ok(Self(field))
    }
}

impl WireEncode for FieldStorageV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.kind() {
            FieldStorageKindV1::ElidedZst {
                exact,
                offset,
                alignment,
            } => {
                prefix(encoder, 1, &exact, offset.get())?;
                encoder.unsigned(alignment.get())
            }
            FieldStorageKindV1::Stored {
                exact,
                offset,
                layout,
            } => {
                prefix(encoder, 2, &exact, offset.get())?;
                layout.encode(encoder)
            }
        }
    }
}

impl WireEncode for DecodedFieldStorageV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match &self.0 {
            RawField::ElidedZst {
                exact,
                offset,
                alignment,
            } => {
                prefix(encoder, 1, exact, *offset)?;
                encoder.unsigned(*alignment)
            }
            RawField::Stored {
                exact,
                offset,
                layout,
            } => {
                prefix(encoder, 2, exact, *offset)?;
                layout.encode(encoder)
            }
        }
    }
}

fn prefix(
    encoder: &mut Encoder,
    tag: u64,
    exact: &impl WireEncode,
    offset: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    exact.encode(encoder)?;
    encoder.field(2)?;
    encoder.unsigned(offset)?;
    encoder.field(3)
}
