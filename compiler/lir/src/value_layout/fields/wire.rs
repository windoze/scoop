use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::*;

/// Raw field references cannot produce storage independently. A reader must
/// first replay the declaring aggregate against its checked layout inputs.
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
        layout: DecodedPersistentId<PersistentLayoutId>,
    },
}

impl DecodedFieldStorageV1 {
    pub(crate) fn link_exact(&self) -> DecodedPersistentId<PersistentExactTypeId> {
        match self.0 {
            RawField::ElidedZst { exact, .. } | RawField::Stored { exact, .. } => exact,
        }
    }

    pub fn validate_against(
        self,
        expected: &FieldStorageV1,
    ) -> Result<FieldStorageV1, StorageReplayError> {
        let equal = match (self.0, expected.kind()) {
            (
                RawField::ElidedZst {
                    exact,
                    offset,
                    alignment,
                },
                FieldStorageKindV1::ElidedZst {
                    exact: expected_exact,
                    offset: expected_offset,
                    alignment: expected_alignment,
                },
            ) => {
                exact.verify(expected_exact).is_ok()
                    && offset == expected_offset.get()
                    && alignment == expected_alignment.get()
            }
            (
                RawField::Stored {
                    exact,
                    offset,
                    layout,
                },
                FieldStorageKindV1::Stored {
                    exact: expected_exact,
                    offset: expected_offset,
                    layout: expected_layout,
                },
            ) => {
                exact.verify(expected_exact).is_ok()
                    && offset == expected_offset.get()
                    && layout.verify(expected_layout.layout()).is_ok()
            }
            _ => false,
        };
        if equal {
            Ok(expected.clone())
        } else {
            Err(StorageReplayError::FieldWireMismatch)
        }
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
                layout: decoder.field(3, DecodedPersistentId::decode)?,
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
                layout.layout().encode(encoder)
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
